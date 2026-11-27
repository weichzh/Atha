//! Budget the inputs to boko 0.5's raw index parser and KF8 reassembly.
//! This checks lengths and counts only; boko remains the index/TOC renderer.

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};

use super::{
    ImportError, MAX_MANIFEST_SECTIONS, MAX_MANIFEST_TOC_ITEMS, MAX_RESOURCE_BYTES, MAX_TEXT_BYTES,
    NULL_INDEX, be_u16, be_u32,
};

const MAX_STRUCTURE_RECORD: usize = 128 * 1024;
const MAX_STRUCTURE_BYTES: usize = 4 * 1024 * 1024;
const MAX_INDEX_RECORDS: usize = 128;
const MAX_TAGS: usize = 32;
const MAX_TAG_VALUES: usize = 32;
const MAX_CNCX_LABEL: usize = 4 * 1024;
const MAX_DIVS: usize = 16_384;
const MAX_CNCX_LABELS: usize = MAX_DIVS + MAX_MANIFEST_TOC_ITEMS;
const MAX_TOC_DEPTH: usize = 64;
const MAX_REASSEMBLY_COPY_BYTES: usize = 256 * 1024 * 1024;

struct Records<'a> {
    file: &'a mut File,
    offsets: &'a [u64], // last offset is EOF
    text_records: usize,
    remaining: usize,
    labels_left: usize,
}

impl Records<'_> {
    fn read(&mut self, index: usize) -> Result<Vec<u8>, ImportError> {
        if index <= self.text_records || index >= self.offsets.len() - 1 {
            return Err(ImportError::InvalidStructure);
        }
        let start = self.offsets[index];
        let len = (self.offsets[index + 1] - start) as usize;
        if len > MAX_STRUCTURE_RECORD {
            return Err(ImportError::TextTooLarge);
        }
        self.remaining = self
            .remaining
            .checked_sub(len)
            .ok_or(ImportError::TextTooLarge)?;
        let mut bytes = vec![0; len];
        self.file
            .seek(SeekFrom::Start(start))
            .and_then(|_| self.file.read_exact(&mut bytes))
            .map_err(|_| ImportError::InvalidStructure)?;
        Ok(bytes)
    }

    fn index(
        &mut self,
        index: Option<usize>,
        max_entries: usize,
    ) -> Result<Vec<Entry>, ImportError> {
        let Some(index) = index else {
            return Ok(Vec::new());
        };
        let header = self.read(index)?;
        check_indx(&header)?;
        let records = be_u32(&header, 24)? as usize;
        let strings = be_u32(&header, 52)? as usize;
        if records
            .checked_add(strings)
            .is_none_or(|n| n > MAX_INDEX_RECORDS)
        {
            return Err(ImportError::TextTooLarge);
        }
        if index + 1 + records + strings >= self.offsets.len() {
            return Err(ImportError::InvalidStructure);
        }
        let tags_at = be_u32(&header, 180)? as usize;
        let tags_at = if (1..header.len()).contains(&tags_at) {
            tags_at
        } else {
            header
                .windows(4)
                .position(|bytes| bytes == b"TAGX")
                .ok_or(ImportError::InvalidStructure)?
        };
        let tags = Tags::parse(&header[tags_at..])?;
        let mut entries = Vec::new();
        for i in 1..=records {
            let record = self.read(index + i)?;
            check_indx(&record)?;
            let idxt = be_u32(&record, 20)? as usize;
            let count = be_u32(&record, 24)? as usize;
            if count > max_entries - entries.len() {
                return Err(ImportError::TextTooLarge);
            }
            if idxt < 192
                || idxt
                    .checked_add(4 + count * 2)
                    .is_none_or(|end| end > record.len())
                || &record[idxt..idxt + 4] != b"IDXT"
            {
                return Err(ImportError::InvalidStructure);
            }
            for item in 0..count {
                let start = usize::from(be_u16(&record, idxt + 4 + item * 2)?);
                let end = if item + 1 == count {
                    idxt
                } else {
                    usize::from(be_u16(&record, idxt + 6 + item * 2)?)
                };
                if start < 192 || start >= end || end > idxt {
                    return Err(ImportError::InvalidStructure);
                }
                entries.push(tags.entry(&record[start..end])?);
            }
        }
        for i in 0..strings {
            let record = self.read(index + 1 + records + i)?;
            bound_labels(&record, &mut self.labels_left)?;
        }
        Ok(entries)
    }
}

fn check_indx(data: &[u8]) -> Result<(), ImportError> {
    if data.len() < 192 || data.get(..4) != Some(b"INDX") {
        return Err(ImportError::InvalidStructure);
    }
    Ok(())
}

fn bound_labels(mut data: &[u8], labels_left: &mut usize) -> Result<(), ImportError> {
    while !data.is_empty() {
        // Record padding does not create a string entry.
        if data[0] == 0 && data.iter().all(|&byte| byte == 0) {
            break;
        }
        let len = varint(&mut data)? as usize;
        if len > MAX_CNCX_LABEL {
            return Err(ImportError::TextTooLarge);
        }
        if len != 0 {
            debit(labels_left, 1)?;
        }
        data = data.get(len..).ok_or(ImportError::InvalidStructure)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Entry {
    position_from_label: Option<u32>,
    tag_one: u32,
    pair: Option<(u32, u32)>,
    parent: Option<usize>,
}

struct Tags<'a> {
    controls: usize,
    descriptors: &'a [u8],
}

impl Tags<'_> {
    fn parse(data: &[u8]) -> Result<Tags<'_>, ImportError> {
        if data.get(..4) != Some(b"TAGX") {
            return Err(ImportError::InvalidStructure);
        }
        let end = be_u32(data, 4)? as usize;
        let controls = be_u32(data, 8)? as usize;
        if !(1..=MAX_TAGS).contains(&controls)
            || !(12..=12 + 4 * MAX_TAGS).contains(&end)
            || !(end - 12).is_multiple_of(4)
        {
            return Err(ImportError::InvalidStructure);
        }
        let descriptors = data.get(12..end).ok_or(ImportError::InvalidStructure)?;
        Ok(Tags {
            controls,
            descriptors,
        })
    }

    fn entry(&self, data: &[u8]) -> Result<Entry, ImportError> {
        let label_len = usize::from(data[0]);
        let Some((name, remaining)) = data[1..].split_at_checked(label_len) else {
            return Err(ImportError::InvalidStructure);
        };
        let (controls, mut data) = remaining
            .split_at_checked(self.controls)
            .ok_or(ImportError::InvalidStructure)?;
        let mut control = 0;
        let mut pending = [(0, 0, false); MAX_TAGS];
        let mut present = 0;

        // TAGX variable-size tags put their encoded lengths ahead of all
        // values. Mirror this ordering so boko sees the exact bounded spans.
        for tag in self.descriptors.chunks_exact(4) {
            if tag[3] == 1 {
                control += 1;
                continue;
            }
            let mask = tag[2];
            let value = *controls.get(control).ok_or(ImportError::InvalidStructure)? & mask;
            if value == 0 {
                continue;
            }
            let bytes = value == mask && mask.count_ones() > 1;
            let n = if bytes {
                varint(&mut data)? as usize
            } else {
                usize::from(value >> mask.trailing_zeros()) * usize::from(tag[1])
            };
            pending[present] = (tag[0], n, bytes);
            present += 1;
        }

        let mut entry = Entry {
            position_from_label: std::str::from_utf8(name).ok().and_then(|s| s.parse().ok()),
            tag_one: 0,
            pair: None,
            parent: None,
        };
        for (tag, n, bytes) in pending[..present].iter().copied() {
            let mut values = [0; 2];
            let mut count = 0;
            if bytes {
                let (mut values_data, rest) = data
                    .split_at_checked(n)
                    .ok_or(ImportError::InvalidStructure)?;
                data = rest;
                while !values_data.is_empty() {
                    if count == MAX_TAG_VALUES {
                        return Err(ImportError::TextTooLarge);
                    }
                    let value = varint(&mut values_data)?;
                    if let Some(slot) = values.get_mut(count) {
                        *slot = value;
                    }
                    count += 1;
                }
            } else {
                if n > MAX_TAG_VALUES {
                    return Err(ImportError::TextTooLarge);
                }
                for i in 0..n {
                    let value = varint(&mut data)?;
                    if let Some(slot) = values.get_mut(i) {
                        *slot = value;
                    }
                }
            }
            match tag {
                1 => entry.tag_one = values[0],
                6 => entry.pair = Some((values[0], values[1])),
                21 => entry.parent = usize::try_from(values[0] as i32).ok(),
                _ => {}
            }
        }
        Ok(entry)
    }
}

fn varint(data: &mut &[u8]) -> Result<u32, ImportError> {
    let mut value = 0_u32;
    for _ in 0..5 {
        let (&byte, remaining) = data.split_first().ok_or(ImportError::InvalidStructure)?;
        *data = remaining;
        value = value
            .checked_mul(128)
            .ok_or(ImportError::InvalidStructure)?
            | u32::from(byte & 0x7f);
        if byte & 0x80 != 0 {
            return Ok(value);
        }
    }
    Err(ImportError::InvalidStructure)
}

fn field(header: &[u8], offset: usize) -> Option<usize> {
    be_u32(header, offset)
        .ok()
        .filter(|&index| index != NULL_INDEX)
        .map(|index| index as usize)
}

pub(super) fn validate(
    file: &mut File,
    offsets: &[u64],
    text_records: usize,
    header: &[u8],
    kf8: bool,
    text_bytes: usize,
) -> Result<super::projection::Layout, ImportError> {
    let mut records = Records {
        file,
        offsets,
        text_records,
        remaining: MAX_STRUCTURE_BYTES,
        labels_left: MAX_CNCX_LABELS,
    };
    let toc = records.index(field(header, 0xf4), MAX_MANIFEST_TOC_ITEMS)?;
    validate_toc(&toc)?;
    if !kf8 {
        return Ok(super::projection::Layout::Mobi { ncx: toc.len() });
    }
    let fdst = (header.len() >= 0xc8)
        .then(|| field(header, 0xc0))
        .flatten();
    let html_bytes = match fdst {
        Some(index) => flow_zero(&records.read(index)?, text_bytes)?,
        None => text_bytes,
    };
    let (divs, skels) = if header.len() >= 0x108 {
        (
            records.index(field(header, 0xf8), MAX_DIVS)?,
            records.index(field(header, 0xfc), MAX_MANIFEST_SECTIONS)?,
        )
    } else {
        (Vec::new(), Vec::new())
    };
    if divs.iter().any(|entry| entry.position_from_label.is_none()) {
        return Err(ImportError::InvalidStructure);
    }
    for item in toc {
        if let Some((fid, off)) = item.pair
            && let Some(div) = divs.get(fid as usize)
            && div
                .position_from_label
                .and_then(|position| position.checked_add(off))
                .is_none()
        {
            return Err(ImportError::InvalidStructure);
        }
    }
    reassembly_budget(
        &skels,
        &divs,
        html_bytes,
        MAX_RESOURCE_BYTES as usize,
        MAX_TEXT_BYTES as usize,
        MAX_REASSEMBLY_COPY_BYTES,
    )
    .map(super::projection::Layout::Kf8)
}

fn flow_zero(data: &[u8], text_bytes: usize) -> Result<usize, ImportError> {
    if data.get(..4) != Some(b"FDST") {
        return Err(ImportError::InvalidStructure);
    }
    let table = be_u32(data, 4)? as usize;
    let count = be_u32(data, 8)? as usize;
    if count > 1024
        || table
            .checked_add(count * 8)
            .is_none_or(|end| end > data.len())
    {
        return Err(ImportError::InvalidStructure);
    }
    if count == 0 {
        return Ok(text_bytes);
    }
    let start = be_u32(data, table)? as usize;
    let end = be_u32(data, table + 4)? as usize;
    // boko indexes SKEL, DIV and NCX relative to the primary HTML flow.
    if start != 0 || end > text_bytes {
        return Err(ImportError::InvalidStructure);
    }
    Ok(end)
}

fn validate_toc(entries: &[Entry]) -> Result<(), ImportError> {
    for item in entries {
        let mut cursor = item.parent;
        for _ in 0..MAX_TOC_DEPTH {
            cursor = match cursor {
                Some(index) => {
                    entries
                        .get(index)
                        .ok_or(ImportError::InvalidStructure)?
                        .parent
                }
                None => break,
            };
        }
        if cursor.is_some() {
            return Err(ImportError::InvalidStructure);
        }
    }
    Ok(())
}

fn reassembly_budget(
    skels: &[Entry],
    divs: &[Entry],
    html: usize,
    section_limit: usize,
    mut total_left: usize,
    mut copy_left: usize,
) -> Result<Vec<super::projection::Part>, ImportError> {
    let mut parts = Vec::with_capacity(skels.len());
    let mut next_div = 0_usize;
    let mut previous_start = 0;
    for skel in skels {
        let (start, len) = skel.pair.unwrap_or_default();
        let (start, len) = (start as usize, len as usize);
        let mut size = len;
        if start < previous_start {
            return Err(ImportError::InvalidStructure);
        }
        previous_start = start;
        debit(&mut copy_left, size)?;
        let stop_div = next_div
            .checked_add(skel.tag_one as usize)
            .ok_or(ImportError::InvalidStructure)?;
        let assigned = divs
            .get(next_div..stop_div)
            .ok_or(ImportError::InvalidStructure)?;
        let mut insertions = 0;
        for div in assigned {
            let position = div
                .position_from_label
                .ok_or(ImportError::InvalidStructure)?;
            let insert = (position as usize).saturating_sub(start);
            if insert > size {
                return Err(ImportError::InvalidStructure);
            }
            let bytes = div.pair.unwrap_or_default().1 as usize;
            size = size.checked_add(bytes).ok_or(ImportError::TextTooLarge)?;
            insertions += usize::from(bytes != 0);
            // boko copies the entire growing chapter once for each insertion.
            debit(&mut copy_left, size)?;
        }
        if start.checked_add(size).is_none_or(|end| end > html) {
            return Err(ImportError::InvalidStructure);
        }
        if size > section_limit {
            return Err(ImportError::TextTooLarge);
        }
        debit(&mut total_left, size)?;
        // boko reads SKEL followed by each assigned DIV from this span. It
        // reorders those bytes at insertion points, but does not add any here.
        parts.push(super::projection::Part {
            start,
            bytes: size,
            insertions,
        });
        next_div = stop_div;
    }
    Ok(parts)
}

fn debit(left: &mut usize, bytes: usize) -> Result<(), ImportError> {
    *left = left.checked_sub(bytes).ok_or(ImportError::TextTooLarge)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_skeleton_count_and_span_from_tagx() {
        let tagx = b"TAGX\0\0\0\x18\0\0\0\x01\x01\x01\x01\0\x06\x02\x02\0\0\0\0\x01";
        let tags = Tags::parse(tagx).unwrap();
        let entry = tags.entry(b"\x03doc\x03\x81\x80\x88").unwrap();
        assert_eq!(entry.tag_one, 1);
        assert_eq!(entry.pair, Some((0, 8)));
    }

    #[test]
    fn reassembly_counts_insertions_in_output_and_copied_bytes() {
        let skel = Entry {
            position_from_label: None,
            tag_one: 1,
            pair: Some((0, 8)),
            parent: None,
        };
        let div = Entry {
            position_from_label: Some(4),
            tag_one: 0,
            pair: Some((8, 4)),
            parent: None,
        };
        let check = |section, book, copied| {
            reassembly_budget(&[skel], &[div], 12, section, book, copied).map(|_| ())
        };
        assert_eq!(check(12, 12, 20), Ok(()));
        assert_eq!(check(11, 12, 20), Err(ImportError::TextTooLarge));
        assert_eq!(check(12, 11, 20), Err(ImportError::TextTooLarge));
        assert_eq!(check(12, 12, 19), Err(ImportError::TextTooLarge));
    }
}
