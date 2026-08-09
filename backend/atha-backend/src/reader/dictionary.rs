//! Bounded local dictionaries with static format dispatch.

use std::{
    error::Error,
    fmt, fs,
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use dom_query::Document;
use encoding_rs::WINDOWS_1252;
use mdict_rs::{MddFile, MdxFile};
use serde::{Deserialize, Serialize};

use super::source::{SourceDigest, SourceError, hash_file};

const DICTIONARY_SCHEMA: u8 = 1;
const MDICT_IDENTITY_DOMAIN: &[u8] = b"atha/dictionary/mdict-rs-0.1.4-v2\0";
const MDICT_PART_IDENTITY_DOMAIN: &[u8] = b"atha/dictionary/mdict-part-v1\0";
const KINDLE_IDENTITY_DOMAIN: &[u8] = b"atha/dictionary/kindle-mobi6-v1\0";
const RECORD_FILE: &str = "dictionary.json";
const MDX_FILE: &str = "dictionary.mdx";
const KINDLE_FILE: &str = "dictionary.mobi";
const KINDLE_OFFSETS_FILE: &str = "dictionary.offsets";
const KINDLE_OFFSETS_MAGIC: &[u8; 8] = b"ATHAKO1\0";
const MAX_DICTIONARY_BYTES: u64 = 256 * 1024 * 1024;
const MAX_RESOURCE_FILES: usize = 4;
const MAX_QUERY_CHARS: usize = 128;
const MAX_LINK_DEPTH: usize = 8;
const MAX_RAW_DEFINITION_BYTES: usize = 2 * 1024 * 1024;
const MAX_DEFINITION_CHARS: usize = 128 * 1024;
const MAX_RESOURCE_BYTES: usize = 32 * 1024 * 1024;
const MAX_TITLE_CHARS: usize = 512;
const NULL_INDEX: u32 = u32::MAX;
const MAX_KINDLE_RECORDS: usize = 100_000;
const MAX_KINDLE_TEXT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_KINDLE_INDEX_RECORDS: usize = 4_096;
const MAX_KINDLE_INDEX_ENTRIES: usize = 2_000_000;
const MAX_KINDLE_ENTRIES_PER_RECORD: usize = 10_000;
const MAX_KINDLE_HUFF_RECORDS: usize = 512;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DictionaryFormat {
    KindleMobi6,
    Mdict,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDictionary {
    pub id: String,
    pub title: String,
    pub format: DictionaryFormat,
    pub entry_count: u64,
    pub resource_count: usize,
    pub imported_at: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryLookup {
    pub dictionary_id: String,
    pub headword: String,
    pub definition: String,
}

#[derive(Clone, Debug)]
pub struct LocalDictionaries {
    root: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DictionaryError {
    InvalidRoot,
    InvalidSource,
    SourceTooLarge,
    TooManyResources,
    Unsupported,
    CorruptSource,
    InvalidDictionaryId,
    UnknownDictionary,
    CorruptRecord,
    InvalidQuery,
    DefinitionTooLarge,
    ResourceTooLarge,
    LinkDepth,
    ReadFailed,
    WriteFailed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredDictionary {
    schema: u8,
    id: String,
    title: String,
    format: DictionaryFormat,
    entry_count: u64,
    resource_count: usize,
    imported_at: u64,
}

struct KindleDictionary {
    file: File,
    offsets: Vec<u64>,
    text_bytes: u64,
    text_records: usize,
    extra_data_flags: u16,
    index_start: usize,
    index_records: usize,
    tagx: Vec<Tagx>,
    control_byte_count: usize,
    huff_index: usize,
    huff_count: usize,
    title: String,
    entry_count: u64,
    text_offsets: Option<Vec<u64>>,
}

#[derive(Clone, Copy)]
struct Tagx {
    tag: u8,
    values_count: u8,
    bitmask: u8,
    control_byte: u8,
}

struct KindleIndexEntry {
    label: String,
    position: u64,
    length: u64,
}

impl KindleDictionary {
    fn open(path: &Path) -> Result<Self, DictionaryError> {
        let mut file = File::open(path).map_err(|_| DictionaryError::InvalidSource)?;
        let file_bytes = file
            .metadata()
            .map_err(|_| DictionaryError::InvalidSource)?
            .len();
        if !(94..=MAX_DICTIONARY_BYTES).contains(&file_bytes) {
            return Err(if file_bytes > MAX_DICTIONARY_BYTES {
                DictionaryError::SourceTooLarge
            } else {
                DictionaryError::CorruptSource
            });
        }
        let header = read_at(&mut file, 0, 78)?;
        if header.get(60..68) != Some(b"BOOKMOBI") {
            return Err(DictionaryError::Unsupported);
        }
        let record_count = usize::from(be_u16(&header, 76)?);
        if !(2..=MAX_KINDLE_RECORDS).contains(&record_count) {
            return Err(DictionaryError::CorruptSource);
        }
        let table_bytes = record_count
            .checked_mul(8)
            .ok_or(DictionaryError::CorruptSource)?;
        let table = read_at(&mut file, 78, table_bytes)?;
        let mut offsets = table
            .chunks_exact(8)
            .map(|entry| be_u32(entry, 0).map(u64::from))
            .collect::<Result<Vec<_>, _>>()?;
        let table_end = 78_u64 + table_bytes as u64;
        if offsets.first().is_none_or(|offset| *offset < table_end)
            || offsets
                .windows(2)
                .any(|pair| pair[0] >= pair[1] || pair[1] >= file_bytes)
            || offsets.last().is_none_or(|offset| *offset >= file_bytes)
        {
            return Err(DictionaryError::CorruptSource);
        }
        offsets.push(file_bytes);
        let record_zero = read_record_bytes(&mut file, &offsets, 0)?;
        if record_zero.get(16..20) != Some(b"MOBI")
            || be_u16(&record_zero, 0)? != 0x4448
            || be_u16(&record_zero, 12)? != 0
            || be_u32(&record_zero, 28)? != 1252
            || !(1..8).contains(&be_u32(&record_zero, 36)?)
        {
            return Err(DictionaryError::Unsupported);
        }
        let text_bytes = u64::from(be_u32(&record_zero, 4)?);
        let text_records = usize::from(be_u16(&record_zero, 8)?);
        let record_size = usize::from(be_u16(&record_zero, 10)?);
        if text_bytes == 0
            || text_bytes > MAX_KINDLE_TEXT_BYTES
            || text_records == 0
            || text_records >= record_count
            || record_size == 0
        {
            return Err(DictionaryError::CorruptSource);
        }
        let orth_index = usize::try_from(be_u32(&record_zero, 40)?)
            .map_err(|_| DictionaryError::CorruptSource)?;
        let huff_index = usize::try_from(be_u32(&record_zero, 0x70)?)
            .map_err(|_| DictionaryError::CorruptSource)?;
        let huff_count = usize::try_from(be_u32(&record_zero, 0x74)?)
            .map_err(|_| DictionaryError::CorruptSource)?;
        if orth_index == NULL_INDEX as usize
            || huff_index == NULL_INDEX as usize
            || !(2..=MAX_KINDLE_HUFF_RECORDS).contains(&huff_count)
            || huff_index
                .checked_add(huff_count)
                .is_none_or(|end| end > record_count)
        {
            return Err(DictionaryError::CorruptSource);
        }
        let primary = read_record_bytes(&mut file, &offsets, orth_index)?;
        let (index_records, entry_count, control_byte_count, tagx) = parse_primary_index(&primary)?;
        let index_start = orth_index
            .checked_add(1)
            .ok_or(DictionaryError::CorruptSource)?;
        if !(1..=MAX_KINDLE_INDEX_RECORDS).contains(&index_records)
            || index_start
                .checked_add(index_records)
                .is_none_or(|end| end > record_count)
            || entry_count == 0
            || entry_count > MAX_KINDLE_INDEX_ENTRIES as u64
        {
            return Err(DictionaryError::CorruptSource);
        }
        let title = mobi_title(&record_zero)
            .or_else(|| pdb_title(&header))
            .unwrap_or_else(|| "Kindle 词典".into());
        let extra_data_flags = be_u16(&record_zero, 0xF2).unwrap_or(0);
        Ok(Self {
            file,
            offsets,
            text_bytes,
            text_records,
            extra_data_flags,
            index_start,
            index_records,
            tagx,
            control_byte_count,
            huff_index,
            huff_count,
            title,
            entry_count,
            text_offsets: None,
        })
    }

    fn open_imported(path: &Path) -> Result<Self, DictionaryError> {
        let mut dictionary = Self::open(path)?;
        let offsets = path.with_file_name(KINDLE_OFFSETS_FILE);
        if offsets.is_file() {
            dictionary.text_offsets = Some(read_kindle_offsets(
                &offsets,
                dictionary.text_records,
                dictionary.text_bytes,
            )?);
        } else {
            dictionary.ensure_text_offsets()?;
            let _ = write_kindle_offsets(
                &offsets,
                dictionary
                    .text_offsets
                    .as_deref()
                    .ok_or(DictionaryError::CorruptSource)?,
                dictionary.text_records,
                dictionary.text_bytes,
            );
        }
        Ok(dictionary)
    }

    fn lookup(
        &mut self,
        dictionary_id: &str,
        query: &str,
    ) -> Result<Option<DictionaryLookup>, DictionaryError> {
        let target = query.to_lowercase();
        let mut low = 0;
        let mut high = self.index_records;
        while low < high {
            let middle = low + (high - low) / 2;
            let label = self.first_label(middle)?.to_lowercase();
            if label.as_str() <= target.as_str() {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        let record = low.saturating_sub(1);
        let data = read_record_bytes(&mut self.file, &self.offsets, self.index_start + record)?;
        let Some(entry) = parse_index_entries(&data, &self.tagx, self.control_byte_count)?
            .into_iter()
            .find(|entry| entry.label.to_lowercase() == target)
        else {
            return Ok(None);
        };
        let raw = self.read_definition(entry.position, entry.length)?;
        let (text, _, had_errors) = WINDOWS_1252.decode(&raw);
        if had_errors {
            return Err(DictionaryError::CorruptSource);
        }
        Ok(Some(DictionaryLookup {
            dictionary_id: dictionary_id.into(),
            headword: entry.label,
            definition: safe_definition(&text)?,
        }))
    }

    fn first_label(&mut self, relative_record: usize) -> Result<String, DictionaryError> {
        let data = read_record_bytes(
            &mut self.file,
            &self.offsets,
            self.index_start + relative_record,
        )?;
        let offsets = index_entry_offsets(&data)?;
        parse_index_label(
            &data,
            *offsets.first().ok_or(DictionaryError::CorruptSource)?,
            *offsets.get(1).ok_or(DictionaryError::CorruptSource)?,
        )
    }

    fn read_definition(&mut self, position: u64, length: u64) -> Result<Vec<u8>, DictionaryError> {
        let end = position
            .checked_add(length)
            .ok_or(DictionaryError::CorruptSource)?;
        if length == 0 || length > MAX_RAW_DEFINITION_BYTES as u64 || end > self.text_bytes {
            return Err(DictionaryError::DefinitionTooLarge);
        }
        self.ensure_text_offsets()?;
        let offsets = self
            .text_offsets
            .as_ref()
            .ok_or(DictionaryError::CorruptSource)?;
        let first = offsets
            .partition_point(|offset| *offset <= position)
            .saturating_sub(1);
        let last = offsets
            .partition_point(|offset| *offset < end)
            .saturating_sub(1);
        if first > last || last >= self.text_records {
            return Err(DictionaryError::CorruptSource);
        }
        let huff = read_record_bytes(&mut self.file, &self.offsets, self.huff_index)?;
        let mut cdics = Vec::with_capacity(self.huff_count - 1);
        for index in 1..self.huff_count {
            cdics.push(read_record_bytes(
                &mut self.file,
                &self.offsets,
                self.huff_index + index,
            )?);
        }
        let cdic_refs = cdics.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let mut huff =
            HuffCdicReader::new(&huff, &cdic_refs).map_err(|_| DictionaryError::CorruptSource)?;
        let mut budget = MAX_RAW_DEFINITION_BYTES;
        let mut output = Vec::with_capacity(length as usize);
        for record_index in first..=last {
            let compressed = read_record_bytes(&mut self.file, &self.offsets, record_index + 1)?;
            let decompressed = huff
                .decompress(
                    strip_trailing_data(&compressed, self.extra_data_flags),
                    &mut budget,
                )
                .map_err(|_| DictionaryError::CorruptSource)?;
            let record_start = offsets[record_index];
            if decompressed.len() as u64 != offsets[record_index + 1] - record_start {
                return Err(DictionaryError::CorruptSource);
            }
            let slice_start = position.saturating_sub(record_start) as usize;
            let slice_end = usize::try_from(end.saturating_sub(record_start))
                .unwrap_or(usize::MAX)
                .min(decompressed.len());
            if slice_start > slice_end || slice_start > decompressed.len() {
                return Err(DictionaryError::CorruptSource);
            }
            output.extend_from_slice(&decompressed[slice_start..slice_end]);
        }
        if output.len() != length as usize {
            return Err(DictionaryError::CorruptSource);
        }
        Ok(output)
    }

    fn ensure_text_offsets(&mut self) -> Result<(), DictionaryError> {
        if self.text_offsets.is_some() {
            return Ok(());
        }
        let huff = read_record_bytes(&mut self.file, &self.offsets, self.huff_index)?;
        let mut cdics = Vec::with_capacity(self.huff_count - 1);
        for index in 1..self.huff_count {
            cdics.push(read_record_bytes(
                &mut self.file,
                &self.offsets,
                self.huff_index + index,
            )?);
        }
        let cdic_refs = cdics.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let mut huff =
            HuffCdicReader::new(&huff, &cdic_refs).map_err(|_| DictionaryError::CorruptSource)?;
        let mut offsets = Vec::with_capacity(self.text_records + 1);
        offsets.push(0_u64);
        let decompressed_limit = self
            .text_bytes
            .checked_add(MAX_DECOMPRESSED_TEXT_RECORD as u64)
            .ok_or(DictionaryError::CorruptSource)?;
        for record_index in 1..=self.text_records {
            let compressed = read_record_bytes(&mut self.file, &self.offsets, record_index)?;
            let mut budget = MAX_DECOMPRESSED_TEXT_RECORD;
            let length = huff
                .decompress(
                    strip_trailing_data(&compressed, self.extra_data_flags),
                    &mut budget,
                )
                .map_err(|_| DictionaryError::CorruptSource)?
                .len() as u64;
            let end = offsets
                .last()
                .copied()
                .and_then(|start| start.checked_add(length))
                .filter(|end| *end <= decompressed_limit)
                .ok_or(DictionaryError::CorruptSource)?;
            offsets.push(end);
        }
        if offsets.last().is_none_or(|end| *end < self.text_bytes) {
            return Err(DictionaryError::CorruptSource);
        }
        self.text_offsets = Some(offsets);
        Ok(())
    }
}

fn parse_primary_index(data: &[u8]) -> Result<(usize, u64, usize, Vec<Tagx>), DictionaryError> {
    if data.get(0..4) != Some(b"INDX") {
        return Err(DictionaryError::CorruptSource);
    }
    let header_length =
        usize::try_from(be_u32(data, 4)?).map_err(|_| DictionaryError::CorruptSource)?;
    let index_records =
        usize::try_from(be_u32(data, 24)?).map_err(|_| DictionaryError::CorruptSource)?;
    let encoding = be_u32(data, 28)?;
    let entry_count = u64::from(be_u32(data, 36)?);
    if encoding != 1252
        || header_length < 56
        || data.get(header_length..header_length + 4) != Some(b"TAGX")
        || be_u32(data, 168).unwrap_or(0) != 0
    {
        return Err(DictionaryError::Unsupported);
    }
    let tagx_length = usize::try_from(be_u32(data, header_length + 4)?)
        .map_err(|_| DictionaryError::CorruptSource)?;
    let control_byte_count = usize::try_from(be_u32(data, header_length + 8)?)
        .map_err(|_| DictionaryError::CorruptSource)?;
    if tagx_length < 12
        || !(1..=8).contains(&control_byte_count)
        || (tagx_length - 12) % 4 != 0
        || header_length
            .checked_add(tagx_length)
            .is_none_or(|end| end > data.len())
    {
        return Err(DictionaryError::CorruptSource);
    }
    let tagx = data[header_length + 12..header_length + tagx_length]
        .chunks_exact(4)
        .map(|tag| Tagx {
            tag: tag[0],
            values_count: tag[1],
            bitmask: tag[2],
            control_byte: tag[3],
        })
        .collect::<Vec<_>>();
    if tagx.iter().filter(|tag| tag.control_byte != 0).count() != control_byte_count {
        return Err(DictionaryError::CorruptSource);
    }
    Ok((index_records, entry_count, control_byte_count, tagx))
}

fn parse_index_entries(
    data: &[u8],
    tagx: &[Tagx],
    control_byte_count: usize,
) -> Result<Vec<KindleIndexEntry>, DictionaryError> {
    let offsets = index_entry_offsets(data)?;
    offsets
        .windows(2)
        .map(|pair| parse_index_entry(data, pair[0], pair[1], tagx, control_byte_count))
        .collect()
}

fn index_entry_offsets(data: &[u8]) -> Result<Vec<usize>, DictionaryError> {
    if data.get(0..4) != Some(b"INDX") {
        return Err(DictionaryError::CorruptSource);
    }
    let idxt = usize::try_from(be_u32(data, 20)?).map_err(|_| DictionaryError::CorruptSource)?;
    let count = usize::try_from(be_u32(data, 24)?).map_err(|_| DictionaryError::CorruptSource)?;
    if count == 0
        || count > MAX_KINDLE_ENTRIES_PER_RECORD
        || data.get(idxt..idxt + 4) != Some(b"IDXT")
        || idxt
            .checked_add(4 + count * 2)
            .is_none_or(|end| end > data.len())
    {
        return Err(DictionaryError::CorruptSource);
    }
    let mut offsets = (0..count)
        .map(|index| be_u16(data, idxt + 4 + index * 2).map(usize::from))
        .collect::<Result<Vec<_>, _>>()?;
    offsets.push(idxt);
    if offsets
        .windows(2)
        .any(|pair| pair[0] >= pair[1] || pair[1] > idxt)
    {
        return Err(DictionaryError::CorruptSource);
    }
    Ok(offsets)
}

fn parse_index_label(data: &[u8], start: usize, end: usize) -> Result<String, DictionaryError> {
    let length = usize::from(*data.get(start).ok_or(DictionaryError::CorruptSource)?);
    let label = data
        .get(start + 1..start + 1 + length)
        .filter(|_| start + 1 + length <= end)
        .ok_or(DictionaryError::CorruptSource)?;
    let (decoded, _, had_errors) = WINDOWS_1252.decode(label);
    if had_errors || decoded.contains('\0') {
        return Err(DictionaryError::CorruptSource);
    }
    Ok(decoded.into_owned())
}

fn parse_index_entry(
    data: &[u8],
    start: usize,
    end: usize,
    tagx: &[Tagx],
    control_byte_count: usize,
) -> Result<KindleIndexEntry, DictionaryError> {
    let label = parse_index_label(data, start, end)?;
    let label_bytes = usize::from(data[start]);
    let controls_start = start + 1 + label_bytes;
    let controls_end = controls_start
        .checked_add(control_byte_count)
        .filter(|value| *value <= end)
        .ok_or(DictionaryError::CorruptSource)?;
    let controls = &data[controls_start..controls_end];
    let mut cursor = controls_end;
    let mut control_index = 0;
    let mut pending = Vec::new();
    for descriptor in tagx {
        if descriptor.control_byte != 0 {
            control_index += 1;
            continue;
        }
        let control = *controls
            .get(control_index)
            .ok_or(DictionaryError::CorruptSource)?;
        let mut masked = control & descriptor.bitmask;
        if masked == 0 {
            continue;
        }
        let (count, byte_length) = if masked == descriptor.bitmask {
            if descriptor.bitmask.count_ones() > 1 {
                (None, Some(read_varlen(data, &mut cursor, end)? as usize))
            } else {
                (Some(1_usize), None)
            }
        } else {
            let mut mask = descriptor.bitmask;
            while mask & 1 == 0 {
                mask >>= 1;
                masked >>= 1;
            }
            (Some(masked as usize), None)
        };
        pending.push((descriptor.tag, descriptor.values_count, count, byte_length));
    }
    let mut position = None;
    let mut length = None;
    for (tag, values_count, count, byte_length) in pending {
        let mut values = Vec::new();
        if let Some(count) = count {
            let total = count
                .checked_mul(usize::from(values_count))
                .filter(|value| *value <= 32)
                .ok_or(DictionaryError::CorruptSource)?;
            for _ in 0..total {
                values.push(read_varlen(data, &mut cursor, end)?);
            }
        } else if let Some(byte_length) = byte_length {
            let values_end = cursor
                .checked_add(byte_length)
                .filter(|value| *value <= end)
                .ok_or(DictionaryError::CorruptSource)?;
            while cursor < values_end {
                values.push(read_varlen(data, &mut cursor, values_end)?);
                if values.len() > 32 {
                    return Err(DictionaryError::CorruptSource);
                }
            }
        }
        if tag == 1 {
            position = values.first().copied().map(u64::from);
        } else if tag == 2 {
            length = values.first().copied().map(u64::from);
        }
    }
    Ok(KindleIndexEntry {
        label,
        position: position.ok_or(DictionaryError::CorruptSource)?,
        length: length.ok_or(DictionaryError::CorruptSource)?,
    })
}

fn read_varlen(data: &[u8], cursor: &mut usize, end: usize) -> Result<u32, DictionaryError> {
    let mut value = 0_u32;
    for _ in 0..4 {
        let byte = *data
            .get(*cursor)
            .filter(|_| *cursor < end)
            .ok_or(DictionaryError::CorruptSource)?;
        *cursor += 1;
        value =
            value.checked_shl(7).ok_or(DictionaryError::CorruptSource)? | u32::from(byte & 0x7f);
        if byte & 0x80 != 0 {
            return Ok(value);
        }
    }
    Err(DictionaryError::CorruptSource)
}

fn read_record_bytes(
    file: &mut File,
    offsets: &[u64],
    index: usize,
) -> Result<Vec<u8>, DictionaryError> {
    let start = *offsets.get(index).ok_or(DictionaryError::CorruptSource)?;
    let end = *offsets
        .get(index + 1)
        .ok_or(DictionaryError::CorruptSource)?;
    let length =
        usize::try_from(end.saturating_sub(start)).map_err(|_| DictionaryError::CorruptSource)?;
    if length == 0 || length > 16 * 1024 * 1024 {
        return Err(DictionaryError::CorruptSource);
    }
    read_at(file, start, length)
}

fn read_at(file: &mut File, offset: u64, length: usize) -> Result<Vec<u8>, DictionaryError> {
    let mut bytes = vec![0; length];
    file.seek(SeekFrom::Start(offset))
        .and_then(|_| file.read_exact(&mut bytes))
        .map_err(|_| DictionaryError::CorruptSource)?;
    Ok(bytes)
}

fn be_u16(data: &[u8], offset: usize) -> Result<u16, DictionaryError> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or(DictionaryError::CorruptSource)?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn be_u32(data: &[u8], offset: usize) -> Result<u32, DictionaryError> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or(DictionaryError::CorruptSource)?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn be_u64(data: &[u8], offset: usize) -> Result<u64, DictionaryError> {
    let bytes = data
        .get(offset..offset + 8)
        .ok_or(DictionaryError::CorruptSource)?;
    Ok(u64::from_be_bytes(
        bytes
            .try_into()
            .map_err(|_| DictionaryError::CorruptSource)?,
    ))
}

fn mobi_title(record_zero: &[u8]) -> Option<String> {
    let offset = usize::try_from(be_u32(record_zero, 0x54).ok()?).ok()?;
    let length = usize::try_from(be_u32(record_zero, 0x58).ok()?).ok()?;
    let bytes = record_zero.get(offset..offset.checked_add(length)?)?;
    let (title, _, had_errors) = WINDOWS_1252.decode(bytes);
    (!had_errors)
        .then(|| normalize_text(&title))
        .filter(|title| !title.is_empty())
}

fn pdb_title(header: &[u8]) -> Option<String> {
    let end = header[..32]
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(32);
    let (title, _, had_errors) = WINDOWS_1252.decode(&header[..end]);
    (!had_errors)
        .then(|| normalize_text(&title))
        .filter(|title| !title.is_empty())
}

impl LocalDictionaries {
    pub fn open(data_root: impl AsRef<Path>) -> Result<Self, DictionaryError> {
        let root = data_root.as_ref().join("Dictionaries");
        fs::create_dir_all(&root).map_err(|_| DictionaryError::InvalidRoot)?;
        Ok(Self { root })
    }

    pub fn list(&self) -> Result<Vec<LocalDictionary>, DictionaryError> {
        let mut dictionaries = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|_| DictionaryError::ReadFailed)? {
            let path = entry.map_err(|_| DictionaryError::ReadFailed)?.path();
            if !path.is_dir()
                || path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with('.'))
            {
                continue;
            }
            match read_record(&path) {
                Ok(record) => dictionaries.push(record.public()),
                Err(DictionaryError::UnknownDictionary | DictionaryError::CorruptRecord) => {}
                Err(error) => return Err(error),
            }
        }
        dictionaries.sort_by(|left, right| {
            right
                .imported_at
                .cmp(&left.imported_at)
                .then_with(|| left.title.cmp(&right.title))
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(dictionaries)
    }

    pub fn import_mdict(
        &self,
        mdx: impl AsRef<Path>,
        resources: &[PathBuf],
    ) -> Result<LocalDictionary, DictionaryError> {
        if resources.len() > MAX_RESOURCE_FILES {
            return Err(DictionaryError::TooManyResources);
        }
        let staging = self.root.join(format!(
            ".staging-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| DictionaryError::WriteFailed)?
                .as_nanos()
        ));
        fs::create_dir(&staging).map_err(|_| DictionaryError::WriteFailed)?;
        let result = self.import_mdict_staged(mdx.as_ref(), resources, &staging);
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        result
    }

    pub fn import_kindle(
        &self,
        source: impl AsRef<Path>,
    ) -> Result<LocalDictionary, DictionaryError> {
        let staging = self.root.join(format!(
            ".staging-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| DictionaryError::WriteFailed)?
                .as_nanos()
        ));
        fs::create_dir(&staging).map_err(|_| DictionaryError::WriteFailed)?;
        let result = self.import_kindle_staged(source.as_ref(), &staging);
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        result
    }

    pub fn lookup(
        &self,
        dictionary_id: &str,
        query: &str,
    ) -> Result<Option<DictionaryLookup>, DictionaryError> {
        let query = normalized_query(query)?;
        let directory = self.directory(dictionary_id)?;
        let record = read_record(&directory)?;
        match record.format {
            DictionaryFormat::KindleMobi6 => {
                let mut dictionary = KindleDictionary::open_imported(&directory.join(KINDLE_FILE))?;
                dictionary.lookup(&record.id, &query)
            }
            DictionaryFormat::Mdict => lookup_mdict(&directory, &record.id, &query),
        }
    }

    pub fn resource(
        &self,
        dictionary_id: &str,
        path: &str,
    ) -> Result<Option<Vec<u8>>, DictionaryError> {
        if path.is_empty()
            || path.len() > 1024
            || path.contains('\0')
            || path.split(['/', '\\']).any(|part| part == "..")
        {
            return Err(DictionaryError::InvalidQuery);
        }
        let directory = self.directory(dictionary_id)?;
        let record = read_record(&directory)?;
        if record.format != DictionaryFormat::Mdict {
            return Ok(None);
        }
        for index in 0..record.resource_count {
            let resource = MddFile::open(directory.join(resource_name(index)))
                .map_err(|_| DictionaryError::CorruptSource)?;
            if let Some(span) = resource
                .lookup_span(path)
                .map_err(|_| DictionaryError::CorruptSource)?
            {
                if span.len() > MAX_RESOURCE_BYTES as u64 {
                    return Err(DictionaryError::ResourceTooLarge);
                }
                let mut data = Vec::with_capacity(span.len() as usize);
                resource
                    .read_record_span_with(&span, |chunk| {
                        data.extend_from_slice(chunk);
                        Ok(())
                    })
                    .map_err(|_| DictionaryError::CorruptSource)?;
                if data.len() > MAX_RESOURCE_BYTES {
                    return Err(DictionaryError::ResourceTooLarge);
                }
                return Ok(Some(data));
            }
        }
        Ok(None)
    }

    pub fn remove(&self, dictionary_id: &str) -> Result<(), DictionaryError> {
        let directory = self.directory(dictionary_id)?;
        read_record(&directory)?;
        fs::remove_dir_all(directory).map_err(|_| DictionaryError::WriteFailed)
    }

    fn import_mdict_staged(
        &self,
        mdx: &Path,
        resources: &[PathBuf],
        staging: &Path,
    ) -> Result<LocalDictionary, DictionaryError> {
        let mut budget = SourceDigest::new(b"", MAX_DICTIONARY_BYTES);
        let staged_mdx = staging.join(MDX_FILE);
        copy_source(mdx, &staged_mdx, b"source\0", &mut budget)?;
        let mdx_hash = hash_file(
            &staged_mdx,
            MDICT_PART_IDENTITY_DOMAIN,
            MAX_DICTIONARY_BYTES,
        )
        .map_err(source_error)?;
        let mut resource_hashes = Vec::with_capacity(resources.len());
        for (index, source) in resources.iter().enumerate() {
            let staged_resource = staging.join(resource_name(index));
            copy_source(source, &staged_resource, b"source\0", &mut budget)?;
            resource_hashes.push(
                hash_file(
                    &staged_resource,
                    MDICT_PART_IDENTITY_DOMAIN,
                    MAX_DICTIONARY_BYTES,
                )
                .map_err(source_error)?,
            );
        }
        let id = mdict_identity(&mdx_hash, resource_hashes)?;
        let target = self.root.join(&id);
        if target.exists() {
            fs::remove_dir_all(staging).map_err(|_| DictionaryError::WriteFailed)?;
            return Ok(read_record(&target)?.public());
        }

        let mdx = MdxFile::open(staging.join(MDX_FILE)).map_err(map_mdict_error)?;
        for index in 0..resources.len() {
            MddFile::open(staging.join(resource_name(index))).map_err(map_mdict_error)?;
        }
        let title = mdx
            .header()
            .title
            .as_deref()
            .map(normalize_text)
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| "本地词典".into());
        let record = StoredDictionary {
            schema: DICTIONARY_SCHEMA,
            id: id.clone(),
            title: truncate(&title, MAX_TITLE_CHARS),
            format: DictionaryFormat::Mdict,
            entry_count: mdx.len(),
            resource_count: resources.len(),
            imported_at: now_millis()?,
        };
        validate_record(&record)?;
        write_record(staging, &record)?;
        fs::rename(staging, &target).map_err(|_| DictionaryError::WriteFailed)?;
        Ok(record.public())
    }

    fn import_kindle_staged(
        &self,
        source: &Path,
        staging: &Path,
    ) -> Result<LocalDictionary, DictionaryError> {
        let mut digest = SourceDigest::new(KINDLE_IDENTITY_DOMAIN, MAX_DICTIONARY_BYTES);
        copy_source(source, &staging.join(KINDLE_FILE), b"mobi\0", &mut digest)?;
        let id = digest.finish();
        let target = self.root.join(&id);
        if target.exists() {
            fs::remove_dir_all(staging).map_err(|_| DictionaryError::WriteFailed)?;
            return Ok(read_record(&target)?.public());
        }
        let mut dictionary = KindleDictionary::open(&staging.join(KINDLE_FILE))?;
        dictionary.ensure_text_offsets()?;
        write_kindle_offsets(
            &staging.join(KINDLE_OFFSETS_FILE),
            dictionary
                .text_offsets
                .as_deref()
                .ok_or(DictionaryError::CorruptSource)?,
            dictionary.text_records,
            dictionary.text_bytes,
        )?;
        let record = StoredDictionary {
            schema: DICTIONARY_SCHEMA,
            id: id.clone(),
            title: truncate(&dictionary.title, MAX_TITLE_CHARS),
            format: DictionaryFormat::KindleMobi6,
            entry_count: dictionary.entry_count,
            resource_count: 0,
            imported_at: now_millis()?,
        };
        validate_record(&record)?;
        write_record(staging, &record)?;
        fs::rename(staging, &target).map_err(|_| DictionaryError::WriteFailed)?;
        Ok(record.public())
    }

    fn directory(&self, id: &str) -> Result<PathBuf, DictionaryError> {
        if !valid_id(id) {
            return Err(DictionaryError::InvalidDictionaryId);
        }
        let path = self.root.join(id);
        if !path.is_dir() {
            return Err(DictionaryError::UnknownDictionary);
        }
        Ok(path)
    }
}

impl StoredDictionary {
    fn public(&self) -> LocalDictionary {
        LocalDictionary {
            id: self.id.clone(),
            title: self.title.clone(),
            format: self.format,
            entry_count: self.entry_count,
            resource_count: self.resource_count,
            imported_at: self.imported_at,
        }
    }
}

impl DictionaryError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidRoot => "invalid-dictionary-root",
            Self::InvalidSource => "invalid-dictionary-source",
            Self::SourceTooLarge => "dictionary-source-too-large",
            Self::TooManyResources => "too-many-dictionary-resources",
            Self::Unsupported => "unsupported-dictionary",
            Self::CorruptSource => "corrupt-dictionary-source",
            Self::InvalidDictionaryId => "invalid-dictionary-id",
            Self::UnknownDictionary => "unknown-dictionary",
            Self::CorruptRecord => "corrupt-dictionary-record",
            Self::InvalidQuery => "invalid-dictionary-query",
            Self::DefinitionTooLarge => "dictionary-definition-too-large",
            Self::ResourceTooLarge => "dictionary-resource-too-large",
            Self::LinkDepth => "dictionary-link-depth",
            Self::ReadFailed => "dictionary-read-failed",
            Self::WriteFailed => "dictionary-write-failed",
        }
    }
}

impl fmt::Display for DictionaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl Error for DictionaryError {}

fn lookup_mdict(
    directory: &Path,
    dictionary_id: &str,
    query: &str,
) -> Result<Option<DictionaryLookup>, DictionaryError> {
    let dictionary = MdxFile::open(directory.join(MDX_FILE)).map_err(map_mdict_error)?;
    let mut candidate = query.to_owned();
    for _ in 0..MAX_LINK_DEPTH {
        let Some(record) = dictionary
            .lookup(&candidate)
            .map_err(|_| DictionaryError::CorruptSource)?
        else {
            return Ok(None);
        };
        let raw = record.text.trim();
        if let Some(link) = raw.strip_prefix("@@@LINK=") {
            candidate = normalized_query(link)?;
            continue;
        }
        return Ok(Some(DictionaryLookup {
            dictionary_id: dictionary_id.to_owned(),
            headword: record.key,
            definition: safe_definition(raw)?,
        }));
    }
    Err(DictionaryError::LinkDepth)
}

fn mdict_identity(
    mdx_hash: &str,
    mut resource_hashes: Vec<String>,
) -> Result<String, DictionaryError> {
    resource_hashes.sort_unstable();
    let mut digest = SourceDigest::new(MDICT_IDENTITY_DOMAIN, MAX_DICTIONARY_BYTES);
    digest.update(b"mdx\0").map_err(source_error)?;
    digest.update(mdx_hash.as_bytes()).map_err(source_error)?;
    for hash in resource_hashes {
        digest.update(b"mdd\0").map_err(source_error)?;
        digest.update(hash.as_bytes()).map_err(source_error)?;
    }
    Ok(digest.finish())
}

fn safe_definition(raw: &str) -> Result<String, DictionaryError> {
    if raw.len() > MAX_RAW_DEFINITION_BYTES {
        return Err(DictionaryError::DefinitionTooLarge);
    }
    let document = Document::fragment(raw);
    document
        .select("script,style,noscript,template,iframe,object,embed,form,input,button,select,textarea,video,audio,source,track,link,meta,base")
        .remove();
    let text = document.text();
    let mut normalized = String::new();
    let mut character_count = 0usize;
    for word in text.split_whitespace() {
        if !normalized.is_empty() {
            normalized.push(' ');
            character_count += 1;
        }
        normalized.push_str(word);
        character_count += word.chars().count();
        if character_count > MAX_DEFINITION_CHARS {
            return Err(DictionaryError::DefinitionTooLarge);
        }
    }
    Ok(normalized)
}

fn copy_source(
    source: &Path,
    target: &Path,
    kind: &[u8],
    digest: &mut SourceDigest,
) -> Result<(), DictionaryError> {
    let mut input = File::open(source).map_err(|_| DictionaryError::InvalidSource)?;
    if !input
        .metadata()
        .map_err(|_| DictionaryError::InvalidSource)?
        .is_file()
    {
        return Err(DictionaryError::InvalidSource);
    }
    digest.update(kind).map_err(source_error)?;
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(target)
        .map_err(|_| DictionaryError::WriteFailed)?;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|_| DictionaryError::InvalidSource)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]).map_err(source_error)?;
        output
            .write_all(&buffer[..read])
            .map_err(|_| DictionaryError::WriteFailed)?;
    }
    output.sync_all().map_err(|_| DictionaryError::WriteFailed)
}

fn read_record(directory: &Path) -> Result<StoredDictionary, DictionaryError> {
    let path = directory.join(RECORD_FILE);
    if !path.is_file() {
        return Err(DictionaryError::UnknownDictionary);
    }
    let record = serde_json::from_slice::<StoredDictionary>(
        &fs::read(path).map_err(|_| DictionaryError::ReadFailed)?,
    )
    .map_err(|_| DictionaryError::CorruptRecord)?;
    validate_record(&record)?;
    if directory.file_name().and_then(|value| value.to_str()) != Some(record.id.as_str()) {
        return Err(DictionaryError::CorruptRecord);
    }
    Ok(record)
}

fn write_record(directory: &Path, record: &StoredDictionary) -> Result<(), DictionaryError> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(directory.join(RECORD_FILE))
        .map_err(|_| DictionaryError::WriteFailed)?;
    serde_json::to_writer_pretty(&mut file, record).map_err(|_| DictionaryError::WriteFailed)?;
    file.write_all(b"\n")
        .and_then(|()| file.sync_all())
        .map_err(|_| DictionaryError::WriteFailed)
}

fn write_kindle_offsets(
    path: &Path,
    offsets: &[u64],
    text_records: usize,
    text_bytes: u64,
) -> Result<(), DictionaryError> {
    if offsets.len() != text_records + 1
        || offsets.first() != Some(&0)
        || offsets.windows(2).any(|pair| {
            pair[0] >= pair[1] || pair[1] - pair[0] > MAX_DECOMPRESSED_TEXT_RECORD as u64
        })
        || offsets.last().is_none_or(|end| {
            *end < text_bytes
                || *end > text_bytes.saturating_add(MAX_DECOMPRESSED_TEXT_RECORD as u64)
        })
    {
        return Err(DictionaryError::CorruptSource);
    }
    let text_records = u64::try_from(text_records).map_err(|_| DictionaryError::CorruptSource)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|_| DictionaryError::WriteFailed)?;
    file.write_all(KINDLE_OFFSETS_MAGIC)
        .and_then(|()| file.write_all(&text_records.to_be_bytes()))
        .and_then(|()| file.write_all(&text_bytes.to_be_bytes()))
        .map_err(|_| DictionaryError::WriteFailed)?;
    for offset in offsets {
        file.write_all(&offset.to_be_bytes())
            .map_err(|_| DictionaryError::WriteFailed)?;
    }
    file.sync_all().map_err(|_| DictionaryError::WriteFailed)
}

fn read_kindle_offsets(
    path: &Path,
    text_records: usize,
    text_bytes: u64,
) -> Result<Vec<u64>, DictionaryError> {
    let expected = 24_usize
        .checked_add(
            text_records
                .checked_add(1)
                .and_then(|count| count.checked_mul(8))
                .ok_or(DictionaryError::CorruptSource)?,
        )
        .ok_or(DictionaryError::CorruptSource)?;
    let data = fs::read(path).map_err(|_| DictionaryError::ReadFailed)?;
    if data.len() != expected
        || data.get(0..8) != Some(KINDLE_OFFSETS_MAGIC)
        || be_u64(&data, 8)?
            != u64::try_from(text_records).map_err(|_| DictionaryError::CorruptSource)?
        || be_u64(&data, 16)? != text_bytes
    {
        return Err(DictionaryError::CorruptSource);
    }
    let offsets = data[24..]
        .chunks_exact(8)
        .map(|bytes| u64::from_be_bytes(bytes.try_into().expect("eight-byte offset")))
        .collect::<Vec<_>>();
    if offsets.first() != Some(&0)
        || offsets.last().is_none_or(|end| {
            *end < text_bytes
                || *end > text_bytes.saturating_add(MAX_DECOMPRESSED_TEXT_RECORD as u64)
        })
        || offsets.windows(2).any(|pair| {
            pair[0] >= pair[1] || pair[1] - pair[0] > MAX_DECOMPRESSED_TEXT_RECORD as u64
        })
    {
        return Err(DictionaryError::CorruptSource);
    }
    Ok(offsets)
}

fn validate_record(record: &StoredDictionary) -> Result<(), DictionaryError> {
    if record.schema != DICTIONARY_SCHEMA
        || !valid_id(&record.id)
        || record.title.is_empty()
        || record.title.chars().count() > MAX_TITLE_CHARS
        || record.entry_count == 0
        || record.resource_count > MAX_RESOURCE_FILES
        || record.imported_at == 0
    {
        return Err(DictionaryError::CorruptRecord);
    }
    Ok(())
}

fn normalized_query(value: &str) -> Result<String, DictionaryError> {
    let value = normalize_text(value);
    if value.is_empty() || value.chars().count() > MAX_QUERY_CHARS || value.contains('\0') {
        return Err(DictionaryError::InvalidQuery);
    }
    Ok(value)
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

fn resource_name(index: usize) -> String {
    format!("resource-{index}.mdd")
}

fn valid_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn now_millis() -> Result<u64, DictionaryError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
        .map_err(|_| DictionaryError::WriteFailed)
}

fn source_error(error: SourceError) -> DictionaryError {
    match error {
        SourceError::InvalidSource => DictionaryError::InvalidSource,
        SourceError::SourceTooLarge => DictionaryError::SourceTooLarge,
    }
}

fn map_mdict_error(error: mdict_rs::Error) -> DictionaryError {
    match error {
        mdict_rs::Error::Unsupported(_) => DictionaryError::Unsupported,
        mdict_rs::Error::Io(_) => DictionaryError::InvalidSource,
        _ => DictionaryError::CorruptSource,
    }
}

// Adapted from boko 0.5.0's bounded GPL-3.0-or-later HUFF/CDIC reader.
const MAX_HUFF_DEPTH: usize = 32;
const MAX_HUFF_DICTIONARY_ENTRIES: usize = 2_000_000;
const MAX_DECOMPRESSED_TEXT_RECORD: usize = 64 * 1024;

#[derive(Clone)]
enum HuffEntry {
    Leaf(Vec<u8>),
    Node(Vec<u8>),
    Unpacked(Vec<u8>),
}

struct HuffCdicReader {
    dict1: Vec<(u8, bool, u32)>,
    mincode: Vec<u32>,
    maxcode: Vec<u32>,
    dictionary: Vec<HuffEntry>,
}

impl HuffCdicReader {
    fn new(huff: &[u8], cdics: &[&[u8]]) -> io::Result<Self> {
        let mut reader = Self {
            dict1: Vec::with_capacity(256),
            mincode: Vec::with_capacity(33),
            maxcode: Vec::with_capacity(33),
            dictionary: Vec::new(),
        };
        reader.load_huff(huff)?;
        for cdic in cdics {
            reader.load_cdic(cdic)?;
        }
        Ok(reader)
    }

    fn load_huff(&mut self, huff: &[u8]) -> io::Result<()> {
        if huff.len() < 24 || huff.get(0..8) != Some(b"HUFF\0\0\0\x18") {
            return invalid_huff("invalid HUFF header");
        }
        let off1 = read_huff_u32(huff, 8)? as usize;
        let off2 = read_huff_u32(huff, 12)? as usize;
        if off1.checked_add(256 * 4).is_none_or(|end| end > huff.len())
            || off2.checked_add(64 * 4).is_none_or(|end| end > huff.len())
        {
            return invalid_huff("truncated HUFF table");
        }
        for index in 0..256 {
            let value = read_huff_u32(huff, off1 + index * 4)?;
            let code_length = (value & 0x1f) as u8;
            if code_length == 0 {
                return invalid_huff("zero HUFF code length");
            }
            let maxcode = (((value >> 8).wrapping_add(1)) << (32 - code_length)).wrapping_sub(1);
            self.dict1.push((code_length, value & 0x80 != 0, maxcode));
        }
        self.mincode.push(0);
        self.maxcode.push(0);
        for index in 0..32 {
            let code_length = index + 1;
            let mincode = read_huff_u32(huff, off2 + index * 8)?;
            let maxcode = read_huff_u32(huff, off2 + index * 8 + 4)?;
            self.mincode.push(mincode << (32 - code_length));
            self.maxcode
                .push((maxcode.wrapping_add(1) << (32 - code_length)).wrapping_sub(1));
        }
        Ok(())
    }

    fn load_cdic(&mut self, cdic: &[u8]) -> io::Result<()> {
        if cdic.len() < 16 || cdic.get(0..8) != Some(b"CDIC\0\0\0\x10") {
            return invalid_huff("invalid CDIC header");
        }
        let phrases = read_huff_u32(cdic, 8)? as usize;
        let bits = read_huff_u32(cdic, 12)?;
        if phrases > MAX_HUFF_DICTIONARY_ENTRIES {
            return invalid_huff("too many CDIC entries");
        }
        let block_capacity = 1_usize.checked_shl(bits).unwrap_or(usize::MAX);
        let count = block_capacity.min(phrases.saturating_sub(self.dictionary.len()));
        if self
            .dictionary
            .len()
            .checked_add(count)
            .is_none_or(|value| value > MAX_HUFF_DICTIONARY_ENTRIES)
            || count
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(16))
                .is_none_or(|end| end > cdic.len())
        {
            return invalid_huff("invalid CDIC table");
        }
        for index in 0..count {
            let offset = usize::from(read_huff_u16(cdic, 16 + index * 2)?);
            let entry = 16_usize
                .checked_add(offset)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid CDIC offset"))?;
            let encoded_length = read_huff_u16(cdic, entry)?;
            let length = usize::from(encoded_length & 0x7fff);
            let start = entry + 2;
            let end = start
                .checked_add(length)
                .filter(|end| *end <= cdic.len())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "truncated CDIC entry")
                })?;
            let bytes = cdic[start..end].to_vec();
            self.dictionary.push(if encoded_length & 0x8000 != 0 {
                HuffEntry::Leaf(bytes)
            } else {
                HuffEntry::Node(bytes)
            });
        }
        Ok(())
    }

    fn decompress(&mut self, data: &[u8], shared_budget: &mut usize) -> io::Result<Vec<u8>> {
        let allowed = MAX_DECOMPRESSED_TEXT_RECORD.min(*shared_budget);
        let mut budget = allowed;
        let mut output = Vec::new();
        self.unpack_into(data, &mut output, 0, &mut budget)?;
        *shared_budget -= allowed - budget;
        Ok(output)
    }

    fn unpack_into(
        &mut self,
        data: &[u8],
        output: &mut Vec<u8>,
        depth: usize,
        budget: &mut usize,
    ) -> io::Result<()> {
        if depth > MAX_HUFF_DEPTH {
            return invalid_huff("nested HUFF entry too deep");
        }
        let mut padded = data.to_vec();
        padded.extend_from_slice(&[0; 8]);
        let mut bits_remaining = i64::try_from(data.len().saturating_mul(8)).unwrap_or(i64::MAX);
        let mut position = 0;
        let mut value = read_huff_u64(&padded, position);
        let mut shift = 32_i32;
        while bits_remaining > 0 {
            if shift <= 0 {
                position += 4;
                value = read_huff_u64(&padded, position);
                shift += 32;
            }
            let code = ((value >> shift) & 0xffff_ffff) as u32;
            let (mut code_length, terminal, mut maxcode) = self.dict1[(code >> 24) as usize];
            if !terminal {
                while code_length < 32 && code < self.mincode[code_length as usize] {
                    code_length += 1;
                }
                maxcode = self.maxcode[code_length as usize];
            }
            shift -= i32::from(code_length);
            bits_remaining -= i64::from(code_length);
            if bits_remaining < 0 {
                break;
            }
            let index = (maxcode.wrapping_sub(code) >> (32 - code_length)) as usize;
            let entry = self.dictionary.get(index).cloned().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "HUFF index out of bounds")
            })?;
            match entry {
                HuffEntry::Leaf(bytes) | HuffEntry::Unpacked(bytes) => {
                    take_huff_budget(budget, bytes.len())?;
                    output.extend_from_slice(&bytes);
                }
                HuffEntry::Node(bytes) => {
                    let mut unpacked = Vec::new();
                    self.unpack_into(&bytes, &mut unpacked, depth + 1, budget)?;
                    output.extend_from_slice(&unpacked);
                    self.dictionary[index] = HuffEntry::Unpacked(unpacked);
                }
            }
        }
        Ok(())
    }
}

fn take_huff_budget(budget: &mut usize, bytes: usize) -> io::Result<()> {
    *budget = budget
        .checked_sub(bytes)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "HUFF output exceeds limit"))?;
    Ok(())
}

fn invalid_huff<T>(message: &'static str) -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::InvalidData, message))
}

fn read_huff_u16(data: &[u8], offset: usize) -> io::Result<u16> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "truncated HUFF data"))?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_huff_u32(data: &[u8], offset: usize) -> io::Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "truncated HUFF data"))?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_huff_u64(data: &[u8], offset: usize) -> u64 {
    data.get(offset..offset + 8).map_or(0, |bytes| {
        u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])
    })
}

fn strip_trailing_data(record: &[u8], flags: u16) -> &[u8] {
    if flags == 0 || record.is_empty() {
        return record;
    }
    let mut end = record.len();
    let mut shifted = flags >> 1;
    while shifted != 0 {
        if shifted & 1 != 0 {
            let mut size = 0_usize;
            let mut shift = 0;
            let mut position = end;
            while position > 0 {
                position -= 1;
                let byte = record[position];
                size |= usize::from(byte & 0x7f) << shift;
                shift += 7;
                if byte & 0x80 != 0 || shift >= 28 {
                    break;
                }
            }
            if size > 0 && size <= end {
                end -= size;
            }
        }
        shifted >>= 1;
    }
    if flags & 1 != 0 && end > 0 {
        let overlap = usize::from(record[end - 1] & 3) + 1;
        if overlap <= end {
            end -= overlap;
        }
    }
    &record[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn safe_definition_drops_active_content_and_normalizes_text() {
        assert_eq!(
            safe_definition(
                "<style>.secret{display:none}</style><p>Hello <b>world</b></p><script>steal()</script><form>send me</form>"
            ),
            Ok("Hello world".into())
        );
    }

    #[test]
    fn query_and_resource_paths_are_bounded() {
        assert_eq!(
            normalized_query("  hello\n world  "),
            Ok("hello world".into())
        );
        assert_eq!(normalized_query("  "), Err(DictionaryError::InvalidQuery));
        assert_eq!(
            normalized_query(&"x".repeat(MAX_QUERY_CHARS + 1)),
            Err(DictionaryError::InvalidQuery)
        );
    }

    #[test]
    fn kindle_index_and_huff_headers_are_bounded() {
        assert_eq!(
            index_entry_offsets(b"INDX"),
            Err(DictionaryError::CorruptSource)
        );
        assert!(HuffCdicReader::new(b"HUFF", &[]).is_err());
    }

    #[test]
    fn mdict_identity_does_not_depend_on_resource_selection_order() {
        let first = mdict_identity("mdx", vec!["one".into(), "two".into()]).expect("identity");
        let second = mdict_identity("mdx", vec!["two".into(), "one".into()]).expect("identity");
        assert_eq!(first, second);
    }

    #[test]
    fn list_ignores_incomplete_and_future_records() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(".tmp")
            .join(format!("dictionary-list-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let dictionaries = LocalDictionaries::open(&root).expect("open dictionaries");
        let valid = "0".repeat(64);
        let directory = dictionaries.root.join(&valid);
        fs::create_dir(&directory).expect("create valid directory");
        write_record(
            &directory,
            &StoredDictionary {
                schema: DICTIONARY_SCHEMA,
                id: valid.clone(),
                title: "Valid".into(),
                format: DictionaryFormat::Mdict,
                entry_count: 1,
                resource_count: 0,
                imported_at: 1,
            },
        )
        .expect("write valid record");
        let future_id = "1".repeat(64);
        let future = dictionaries.root.join(&future_id);
        fs::create_dir(&future).expect("create future directory");
        fs::write(
            future.join(RECORD_FILE),
            serde_json::to_vec(&StoredDictionary {
                schema: DICTIONARY_SCHEMA + 1,
                id: future_id,
                title: "Future".into(),
                format: DictionaryFormat::Mdict,
                entry_count: 1,
                resource_count: 0,
                imported_at: 1,
            })
            .expect("serialize future record"),
        )
        .expect("write future record");
        fs::create_dir(dictionaries.root.join("2".repeat(64)))
            .expect("create incomplete directory");

        assert_eq!(dictionaries.list().expect("list").len(), 1);
        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn private_kindle_sample_supports_sparse_exact_lookup() {
        let Some(fixture_root) =
            std::env::var_os("ATHA_PRIVATE_DICTIONARY_ROOT").map(PathBuf::from)
        else {
            return;
        };
        let source = find_private_kindle(&fixture_root).expect("one Kindle dictionary sample");
        let mut raw = KindleDictionary::open(&source).expect("open Kindle dictionary sample");
        let queries = kindle_sample_queries(&mut raw);
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(".tmp")
            .join(format!(
                "dictionary-kindle-test-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("clock after epoch")
                    .as_nanos()
            ));
        fs::create_dir_all(&root).expect("create Kindle test root");
        let dictionaries = LocalDictionaries::open(&root).expect("open dictionaries");
        let imported = dictionaries
            .import_kindle(source)
            .expect("import Kindle dictionary sample");
        for query in queries {
            assert!(
                dictionaries
                    .lookup(&imported.id, &query)
                    .expect("lookup Kindle dictionary sample")
                    .is_some(),
                "sparse ordinal-derived Kindle lookup must resolve"
            );
        }
        fs::remove_dir_all(root).expect("remove Kindle test root");
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct PrivateEnglishEvidence {
        schema: u8,
        kindle: Vec<PrivateEnglishCase>,
        mdict: Vec<PrivateEnglishCase>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    struct PrivateEnglishCase {
        query: String,
        definition_sha256: String,
    }

    #[test]
    fn private_english_dictionary_outputs_are_substantive() {
        let Some(fixture_root) =
            std::env::var_os("ATHA_PRIVATE_DICTIONARY_ROOT").map(PathBuf::from)
        else {
            return;
        };
        let evidence = fs::read(fixture_root.join("dictionary-english-output.json")).expect(
            "private fixture evidence is missing; add the ignored schema-1 English query/hash manifest",
        );
        let evidence = serde_json::from_slice::<PrivateEnglishEvidence>(&evidence)
            .expect("private English evidence manifest is invalid");
        assert!(evidence.schema == 1, "unsupported private evidence schema");
        validate_private_english_cases(&evidence.kindle);
        validate_private_english_cases(&evidence.mdict);

        let kindle_source = find_private_kindle(&fixture_root).expect("one Kindle sample");
        let (mdx_source, mdd_sources) = find_private_mdict(&fixture_root);
        let root = PrivateTestRoot::new("dictionary-english-output");
        let dictionaries = LocalDictionaries::open(&root.0).expect("open dictionaries");
        let kindle = dictionaries
            .import_kindle(kindle_source)
            .expect("import Kindle sample");
        let mdict = dictionaries
            .import_mdict(mdx_source, &mdd_sources)
            .expect("import MDict sample");

        assert_private_english_outputs(&dictionaries, &kindle.id, &evidence.kindle);
        assert_private_english_outputs(&dictionaries, &mdict.id, &evidence.mdict);
    }

    #[test]
    fn kindle_offset_sidecar_round_trips_and_rejects_corruption() {
        let root = PrivateTestRoot::new("dictionary-offset-sidecar");
        let path = root.0.join(KINDLE_OFFSETS_FILE);
        let expected = vec![0, 4, 9];
        write_kindle_offsets(&path, &expected, 2, 9).expect("write Kindle offsets");
        assert_eq!(
            read_kindle_offsets(&path, 2, 9).expect("read Kindle offsets"),
            expected
        );
        fs::write(&path, [KINDLE_OFFSETS_MAGIC.as_slice(), &[0; 32]].concat())
            .expect("corrupt Kindle offsets");
        assert!(read_kindle_offsets(&path, 2, 9).is_err());
    }

    #[test]
    #[ignore = "requires private local dictionary fixtures"]
    fn private_dictionary_benchmark() {
        let fixture_root = std::env::var_os("ATHA_PRIVATE_DICTIONARY_ROOT")
            .map(PathBuf::from)
            .expect("private fixture root");
        let kindle_source = find_private_kindle(&fixture_root).expect("one Kindle sample");
        let mut kindle_raw = KindleDictionary::open(&kindle_source).expect("open Kindle sample");
        let kindle_queries = kindle_sample_queries(&mut kindle_raw);
        let (mdx_source, mdd_sources) = find_private_mdict(&fixture_root);
        let mdx = MdxFile::open(&mdx_source).expect("open MDX sample");
        let mdict_queries = [0, mdx.len() / 2, mdx.len() - 1].map(|ordinal| {
            mdx.key_at(ordinal.into())
                .expect("read MDX key")
                .expect("MDX key")
        });
        let mdd_key = mdd_sources.first().map(|path| {
            let mdd = MddFile::open(path).expect("open MDD sample");
            mdd.key_at((mdd.len() / 2).into())
                .expect("read MDD key")
                .expect("MDD key")
        });
        let root = std::env::var_os("ATHA_DICTIONARY_BENCHMARK_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../..")
                    .join(".tmp")
                    .join(format!("dictionary-benchmark-{}", std::process::id()))
            });
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create benchmark root");
        let dictionaries = LocalDictionaries::open(&root).expect("open dictionaries");
        let kindle = dictionaries
            .import_kindle(kindle_source)
            .expect("import Kindle sample");
        let mdict = dictionaries
            .import_mdict(mdx_source, &mdd_sources)
            .expect("import MDict sample");
        let mut kindle_cold_micros = Vec::new();
        for query in &kindle_queries {
            let started = Instant::now();
            assert!(
                dictionaries
                    .lookup(&kindle.id, query)
                    .expect("cold Kindle")
                    .is_some()
            );
            kindle_cold_micros.push(started.elapsed().as_micros());
        }
        let mut mdict_cold_micros = Vec::new();
        for query in &mdict_queries {
            let started = Instant::now();
            assert!(
                dictionaries
                    .lookup(&mdict.id, query)
                    .expect("cold MDict")
                    .is_some()
            );
            mdict_cold_micros.push(started.elapsed().as_micros());
        }
        let mut resource_cold_micros = Vec::new();
        if let Some(key) = &mdd_key {
            for _ in 0..3 {
                let started = Instant::now();
                assert!(
                    dictionaries
                        .resource(&mdict.id, key)
                        .expect("cold MDD")
                        .is_some()
                );
                resource_cold_micros.push(started.elapsed().as_micros());
            }
        }
        let mut kindle_micros = Vec::new();
        let mut mdict_micros = Vec::new();
        let mut resource_micros = Vec::new();
        for _ in 0..20 {
            for query in &kindle_queries {
                let started = Instant::now();
                assert!(
                    dictionaries
                        .lookup(&kindle.id, query)
                        .expect("Kindle lookup")
                        .is_some()
                );
                kindle_micros.push(started.elapsed().as_micros());
            }
            for query in &mdict_queries {
                let started = Instant::now();
                assert!(
                    dictionaries
                        .lookup(&mdict.id, query)
                        .expect("MDict lookup")
                        .is_some()
                );
                mdict_micros.push(started.elapsed().as_micros());
            }
            if let Some(key) = &mdd_key {
                for _ in 0..3 {
                    let started = Instant::now();
                    assert!(
                        dictionaries
                            .resource(&mdict.id, key)
                            .expect("MDD lookup")
                            .is_some()
                    );
                    resource_micros.push(started.elapsed().as_micros());
                }
            }
        }
        println!(
            "dictionary_benchmark={{\"kindle_entries\":{},\"mdict_entries\":{},\"kindle_cold_lookup_p95_us\":{},\"kindle_hot_lookup_p50_us\":{},\"kindle_hot_lookup_p95_us\":{},\"mdict_cold_lookup_p95_us\":{},\"mdict_hot_lookup_p50_us\":{},\"mdict_hot_lookup_p95_us\":{},\"mdd_cold_lookup_p95_us\":{},\"mdd_hot_lookup_p50_us\":{},\"mdd_hot_lookup_p95_us\":{},\"peak_rss_kib\":{}}}",
            kindle.entry_count,
            mdict.entry_count,
            percentile(&mut kindle_cold_micros, 95),
            percentile(&mut kindle_micros, 50),
            percentile(&mut kindle_micros, 95),
            percentile(&mut mdict_cold_micros, 95),
            percentile(&mut mdict_micros, 50),
            percentile(&mut mdict_micros, 95),
            percentile(&mut resource_cold_micros, 95),
            percentile(&mut resource_micros, 50),
            percentile(&mut resource_micros, 95),
            peak_rss_kib()
        );
        fs::remove_dir_all(root).expect("remove benchmark root");
    }

    #[test]
    #[ignore = "seeds an isolated Linux GUI data root from private fixtures"]
    fn seed_private_dictionary_gui_gate() {
        let fixture_root = std::env::var_os("ATHA_PRIVATE_DICTIONARY_ROOT")
            .map(PathBuf::from)
            .expect("private fixture root");
        let data_root = std::env::var_os("ATHA_DICTIONARY_GATE_DATA_ROOT")
            .map(PathBuf::from)
            .expect("dictionary gate data root");
        let query_path = std::env::var_os("ATHA_DICTIONARY_GATE_QUERY_PATH")
            .map(PathBuf::from)
            .expect("dictionary gate query path");
        let (mdx_source, mdd_sources) = find_private_mdict(&fixture_root);
        let mdx = MdxFile::open(&mdx_source).expect("open MDX sample");
        let query = mdx
            .key_at((mdx.len() / 2).into())
            .expect("read MDX key")
            .expect("MDX key");
        let dictionaries = LocalDictionaries::open(data_root).expect("open gate dictionaries");
        dictionaries
            .import_mdict(mdx_source, &mdd_sources)
            .expect("seed gate dictionary");
        fs::write(
            query_path,
            serde_json::to_vec(&query).expect("serialize private gate query"),
        )
        .expect("write private gate query");
    }

    #[test]
    #[ignore = "stages anonymous private fixtures for the physical Android benchmark"]
    fn stage_private_dictionary_android_fixtures() {
        let fixture_root = std::env::var_os("ATHA_PRIVATE_DICTIONARY_ROOT")
            .map(PathBuf::from)
            .expect("private fixture root");
        let stage_root = std::env::var_os("ATHA_DICTIONARY_ANDROID_STAGE_ROOT")
            .map(PathBuf::from)
            .expect("Android fixture stage root");
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let temporary = fs::canonicalize(repository.join(".tmp")).expect("resolve .tmp root");
        let stage_parent = fs::canonicalize(stage_root.parent().expect("stage parent"))
            .expect("resolve stage parent");
        assert!(stage_root.is_absolute() && stage_parent == temporary);
        let _ = fs::remove_dir_all(&stage_root);
        fs::create_dir(&stage_root).expect("create Android fixture stage");

        let (mdx_source, mdd_sources) = find_private_mdict(&fixture_root);
        fs::copy(mdx_source, stage_root.join("sample.mdx")).expect("stage MDX sample");
        for (index, source) in mdd_sources.iter().enumerate() {
            fs::copy(source, stage_root.join(format!("resource-{index}.mdd")))
                .expect("stage MDD sample");
        }
        fs::copy(
            find_private_kindle(&fixture_root).expect("one Kindle sample"),
            stage_root.join("sample.mobi"),
        )
        .expect("stage Kindle sample");
    }

    fn kindle_sample_queries(dictionary: &mut KindleDictionary) -> [String; 3] {
        let final_record = read_record_bytes(
            &mut dictionary.file,
            &dictionary.offsets,
            dictionary.index_start + dictionary.index_records - 1,
        )
        .expect("read final Kindle index record");
        let tail = parse_index_entries(
            &final_record,
            &dictionary.tagx,
            dictionary.control_byte_count,
        )
        .expect("parse final Kindle index record")
        .pop()
        .expect("final Kindle entry")
        .label;
        [
            dictionary.first_label(0).expect("read first Kindle key"),
            dictionary
                .first_label(dictionary.index_records / 2)
                .expect("read middle Kindle key"),
            tail,
        ]
    }

    fn assert_private_english_outputs(
        dictionaries: &LocalDictionaries,
        dictionary_id: &str,
        cases: &[PrivateEnglishCase],
    ) {
        for case in cases {
            let result = dictionaries
                .lookup(dictionary_id, &case.query)
                .expect("private English lookup")
                .expect("private English evidence query must resolve");
            let query = normalized_english_evidence(&case.query)
                .expect("private English evidence query is invalid");
            let headword = normalized_english_evidence(&result.headword)
                .expect("private English lookup headword is invalid");
            let definition = normalize_text(&result.definition);
            let lower = definition.to_ascii_lowercase();
            assert!(
                result.dictionary_id == dictionary_id,
                "dictionary id mismatch"
            );
            assert!(query == headword, "private English headword mismatch");
            assert!(
                definition.chars().count() >= 24
                    && !matches!(
                        lower.as_str(),
                        "placeholder"
                            | "no definition"
                            | "no definition available"
                            | "undefined"
                            | "null"
                    ),
                "private English definition is empty or a placeholder"
            );
            assert!(
                definition_sha256(&definition) == case.definition_sha256,
                "private English definition digest mismatch"
            );
        }
    }

    fn validate_private_english_cases(cases: &[PrivateEnglishCase]) {
        assert!(
            cases.len() >= 3,
            "private evidence needs at least three cases"
        );
        for (index, case) in cases.iter().enumerate() {
            assert!(
                normalized_english_evidence(&case.query).is_some(),
                "private evidence contains an invalid English query"
            );
            assert!(
                case.definition_sha256.len() == 64
                    && case
                        .definition_sha256
                        .bytes()
                        .all(|byte| { byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte) }),
                "private evidence contains an invalid definition digest"
            );
            assert!(
                cases[..index]
                    .iter()
                    .all(|previous| !previous.query.eq_ignore_ascii_case(&case.query)),
                "private evidence contains duplicate queries"
            );
        }
    }

    fn normalized_english_evidence(value: &str) -> Option<String> {
        let value = normalize_text(value);
        (value.chars().count() <= MAX_QUERY_CHARS
            && value
                .chars()
                .any(|character| character.is_ascii_alphabetic())
            && value.chars().all(|character| {
                character.is_ascii_alphabetic()
                    || character.is_ascii_whitespace()
                    || matches!(character, '-' | '\'' | '.')
            }))
        .then(|| value.to_ascii_lowercase())
    }

    fn definition_sha256(value: &str) -> String {
        let mut digest = SourceDigest::new(b"", MAX_RAW_DEFINITION_BYTES as u64);
        digest
            .update(value.as_bytes())
            .expect("normalized definition exceeds hash budget");
        digest.finish()
    }

    struct PrivateTestRoot(PathBuf);

    impl PrivateTestRoot {
        fn new(label: &str) -> Self {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(".tmp")
                .join(format!(
                    "{label}-{}-{}",
                    std::process::id(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .expect("clock after epoch")
                        .as_nanos()
                ));
            fs::create_dir_all(&path).expect("create private test root");
            Self(path)
        }
    }

    impl Drop for PrivateTestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn find_private_kindle(root: &Path) -> Option<PathBuf> {
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(directory).ok()? {
                let path = entry.ok()?.path();
                if path.is_dir() {
                    pending.push(path);
                    continue;
                }
                let mut file = File::open(&path).ok()?;
                let mut header = [0_u8; 68];
                if file.read_exact(&mut header).is_ok()
                    && &header[60..68] == b"BOOKMOBI"
                    && KindleDictionary::open(&path).is_ok()
                {
                    return Some(path);
                }
            }
        }
        None
    }

    fn find_private_mdict(root: &Path) -> (PathBuf, Vec<PathBuf>) {
        let mut pending = vec![root.to_path_buf()];
        let mut mdx = Vec::new();
        let mut mdd = Vec::new();
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(directory).expect("read fixture directory") {
                let path = entry.expect("read fixture entry").path();
                if path.is_dir() {
                    pending.push(path);
                } else if path
                    .extension()
                    .is_some_and(|value| value.eq_ignore_ascii_case("mdx"))
                {
                    mdx.push(path);
                } else if path
                    .extension()
                    .is_some_and(|value| value.eq_ignore_ascii_case("mdd"))
                {
                    mdd.push(path);
                }
            }
        }
        assert_eq!(mdx.len(), 1, "private root must contain one MDX");
        mdd.sort();
        (mdx.pop().expect("one MDX"), mdd)
    }

    fn percentile(values: &mut [u128], percentile: usize) -> u128 {
        if values.is_empty() {
            return 0;
        }
        values.sort_unstable();
        values[(values.len() * percentile).div_ceil(100).saturating_sub(1)]
    }

    fn peak_rss_kib() -> u64 {
        fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|status| {
                status.lines().find_map(|line| {
                    line.strip_prefix("VmHWM:")?
                        .split_whitespace()
                        .next()?
                        .parse()
                        .ok()
                })
            })
            .unwrap_or(0)
    }
}
