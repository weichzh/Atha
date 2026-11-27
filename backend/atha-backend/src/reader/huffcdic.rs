//! HUFF/CDIC decoding adapted from boko 0.5.0 (GPL-3.0-or-later).

const MAX_DEPTH: usize = 32;
pub(super) const MAX_RECORD_OUTPUT: usize = 64 * 1024;
pub(super) const MAX_CACHE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HuffError {
    InvalidData,
    Limit,
}

enum Entry<'a> {
    Leaf(&'a [u8]),
    Node(&'a [u8]),
    Cached(Vec<u8>),
}

/// The byte allowance applies across records; the optional code allowance
/// also counts zero-output codes and codes decoded inside dictionary phrases.
pub(super) struct Budget {
    pub output_left: usize,
    pub codes_left: Option<usize>,
}

pub(super) struct HuffCdicReader<'a> {
    prefixes: [(u8, bool, u32); 256],
    mincode: [u32; 33],
    maxcode: [u32; 33],
    dictionary: Vec<Entry<'a>>,
    cache_left: usize,
}

impl<'a> HuffCdicReader<'a> {
    pub(super) fn new(
        huff: &[u8],
        cdics: &[&'a [u8]],
        max_entries: usize,
        cache_bytes: usize,
    ) -> Result<Self, HuffError> {
        if huff.get(..8) != Some(b"HUFF\0\0\0\x18") || huff.len() < 24 {
            return Err(HuffError::InvalidData);
        }
        let prefix_table = word_table(huff, 8, 256 * 4)?;
        let code_table = word_table(huff, 12, 32 * 8)?;
        let mut reader = Self {
            prefixes: [(0, false, 0); 256],
            mincode: [0; 33],
            maxcode: [0; 33],
            dictionary: Vec::new(),
            cache_left: cache_bytes,
        };

        for (entry, word) in reader.prefixes.iter_mut().zip(prefix_table.chunks_exact(4)) {
            let word = u32::from_be_bytes(word.try_into().unwrap());
            let length = (word & 0x1f) as u8;
            if length == 0 {
                return Err(HuffError::InvalidData);
            }
            *entry = (length, word & 0x80 != 0, max_code(word >> 8, length));
        }
        for (index, pair) in code_table.chunks_exact(8).enumerate() {
            let length = (index + 1) as u8;
            reader.mincode[index + 1] = read_u32(pair, 0)? << (32 - length);
            reader.maxcode[index + 1] = max_code(read_u32(pair, 4)?, length);
        }

        for cdic in cdics {
            reader.load_cdic(cdic, max_entries)?;
        }
        Ok(reader)
    }

    fn load_cdic(&mut self, cdic: &'a [u8], max_entries: usize) -> Result<(), HuffError> {
        if cdic.get(..8) != Some(b"CDIC\0\0\0\x10") {
            return Err(HuffError::InvalidData);
        }
        let phrases = read_u32(cdic, 8)? as usize;
        if phrases > max_entries {
            return Err(HuffError::Limit);
        }
        let bits = read_u32(cdic, 12)?;
        let count = 1_usize
            .checked_shl(bits)
            .unwrap_or(usize::MAX)
            .min(phrases.saturating_sub(self.dictionary.len()));
        let table = cdic.get(16..16 + count * 2).ok_or(HuffError::InvalidData)?;
        reserve_bounded(&mut self.dictionary, count, max_entries)?;
        for offset in table.chunks_exact(2) {
            let entry = 16 + usize::from(u16::from_be_bytes([offset[0], offset[1]]));
            let size = read_u16(cdic, entry)?;
            let start = entry + 2;
            let end = start + usize::from(size & 0x7fff);
            let bytes = cdic.get(start..end).ok_or(HuffError::InvalidData)?;
            self.dictionary.push(if size & 0x8000 == 0 {
                Entry::Node(bytes)
            } else {
                Entry::Leaf(bytes)
            });
        }
        Ok(())
    }

    pub(super) fn decompress(
        &mut self,
        data: &[u8],
        budget: &mut Budget,
    ) -> Result<Vec<u8>, HuffError> {
        let limit = MAX_RECORD_OUTPUT.min(budget.output_left);
        let mut output = Vec::new();
        self.unpack_into(data, &mut output, 0, limit, &mut budget.codes_left)?;
        budget.output_left -= output.len();
        Ok(output)
    }

    fn unpack_into(
        &mut self,
        data: &[u8],
        output: &mut Vec<u8>,
        depth: usize,
        limit: usize,
        codes_left: &mut Option<usize>,
    ) -> Result<(), HuffError> {
        if depth > MAX_DEPTH {
            return Err(HuffError::Limit);
        }
        let mut bits_remaining = data.len().checked_mul(8).ok_or(HuffError::Limit)?;
        let mut position = 0;
        let mut value = read_window(data);
        let mut shift = 32_i32;
        while bits_remaining != 0 {
            if shift <= 0 {
                position += 4;
                value = read_window(&data[position..]);
                shift += 32;
            }
            let code = (value >> shift) as u32;
            let (mut length, terminal, mut maxcode) = self.prefixes[(code >> 24) as usize];
            if !terminal {
                while length < 32 && code < self.mincode[length as usize] {
                    length += 1;
                }
                maxcode = self.maxcode[length as usize];
            }
            let Some(remaining) = bits_remaining.checked_sub(usize::from(length)) else {
                break;
            };
            bits_remaining = remaining;
            shift -= i32::from(length);
            if let Some(left) = codes_left {
                *left = left.checked_sub(1).ok_or(HuffError::Limit)?;
            }
            let index = (maxcode.wrapping_sub(code) >> (32 - length)) as usize;
            match self.dictionary.get(index).ok_or(HuffError::InvalidData)? {
                Entry::Leaf(bytes) => {
                    reserve_bounded(output, bytes.len(), limit)?;
                    output.extend_from_slice(bytes);
                }
                Entry::Cached(bytes) => {
                    reserve_bounded(output, bytes.len(), limit)?;
                    output.extend_from_slice(bytes);
                }
                Entry::Node(bytes) => {
                    // Expand into the same record buffer: the stack holds only
                    // decoding cursors, not one scratch allocation per depth.
                    let start = output.len();
                    self.unpack_into(bytes, output, depth + 1, limit, codes_left)?;
                    let expansion = &output[start..];
                    if let Some(left) = self.cache_left.checked_sub(expansion.len()) {
                        let mut cached = Vec::new();
                        cached
                            .try_reserve_exact(expansion.len())
                            .map_err(|_| HuffError::Limit)?;
                        cached.extend_from_slice(expansion);
                        self.cache_left = left;
                        self.dictionary[index] = Entry::Cached(cached);
                    }
                }
            }
        }
        Ok(())
    }
}

fn reserve_bounded<T>(data: &mut Vec<T>, additional: usize, limit: usize) -> Result<(), HuffError> {
    let needed = data
        .len()
        .checked_add(additional)
        .filter(|needed| *needed <= limit)
        .ok_or(HuffError::Limit)?;
    if needed > data.capacity() {
        let capacity = needed.max(data.capacity().saturating_mul(2).min(limit));
        data.try_reserve_exact(capacity - data.len())
            .map_err(|_| HuffError::Limit)?;
    }
    Ok(())
}

fn word_table(data: &[u8], pointer: usize, size: usize) -> Result<&[u8], HuffError> {
    let offset = read_u32(data, pointer)? as usize;
    offset
        .checked_add(size)
        .and_then(|end| data.get(offset..end))
        .ok_or(HuffError::InvalidData)
}

fn max_code(raw: u32, length: u8) -> u32 {
    (raw.wrapping_add(1) << (32 - length)).wrapping_sub(1)
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16, HuffError> {
    let bytes = data.get(offset..offset + 2).ok_or(HuffError::InvalidData)?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, HuffError> {
    let bytes = data.get(offset..offset + 4).ok_or(HuffError::InvalidData)?;
    Ok(u32::from_be_bytes(bytes.try_into().unwrap()))
}

/// Up to eight bytes of lookahead, zero-padded without copying the input.
fn read_window(data: &[u8]) -> u64 {
    let mut padded = [0; 8];
    let len = data.len().min(padded.len());
    padded[..len].copy_from_slice(&data[..len]);
    u64::from_be_bytes(padded)
}

pub(super) fn strip_trailing_data(record: &[u8], flags: u16) -> &[u8] {
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

    fn sample(cache_bytes: usize) -> HuffCdicReader<'static> {
        HuffCdicReader {
            prefixes: [(8, true, u32::MAX); 256],
            mincode: [0; 33],
            maxcode: [0; 33],
            dictionary: vec![
                Entry::Leaf(b"Hello"),
                Entry::Leaf(b"!"),
                Entry::Node(&[255, 254]),
            ],
            cache_left: cache_bytes,
        }
    }

    #[test]
    fn cached_and_uncached_phrases_share_the_actual_output_budget() {
        for cache_budget in [5, 6] {
            let mut reader = sample(cache_budget);
            let mut budget = Budget {
                output_left: 12,
                codes_left: Some(6),
            };
            assert_eq!(reader.decompress(&[253], &mut budget).unwrap(), b"Hello!");
            assert_eq!(budget.output_left, 6);
            let cached = matches!(reader.dictionary[2], Entry::Cached(_));
            assert_eq!(cached, cache_budget == 6);
            assert_eq!(reader.cache_left, if cached { 0 } else { cache_budget });
            assert_eq!(reader.decompress(&[253], &mut budget).unwrap(), b"Hello!");
            assert_eq!(budget.output_left, 0);
            assert_eq!(budget.codes_left, Some(if cached { 2 } else { 0 }));
            budget.codes_left = Some(1);
            assert_eq!(
                reader.decompress(&[255], &mut budget),
                Err(HuffError::Limit)
            );
        }
    }

    #[test]
    fn actual_bytes_and_decoded_codes_are_checked_before_append() {
        let mut reader = sample(6);
        let mut output = Vec::new();
        assert_eq!(
            reader.unpack_into(&[255, 254], &mut output, 0, 5, &mut Some(2)),
            Err(HuffError::Limit)
        );
        assert_eq!(output, b"Hello");
        assert!(output.capacity() <= 5);

        let mut output = Vec::new();
        assert_eq!(
            reader.unpack_into(&[253], &mut output, 0, 6, &mut Some(2)),
            Err(HuffError::Limit)
        );
        assert_eq!(output, b"Hello");
        assert_eq!(reader.cache_left, 6);
    }

    #[test]
    fn actual_record_limit_is_separate_from_the_remaining_book_budget() {
        let mut reader = sample(6);
        let mut budget = Budget {
            output_left: MAX_RECORD_OUTPUT * 3,
            codes_left: None,
        };
        let mut bytes = vec![254; MAX_RECORD_OUTPUT];
        assert_eq!(
            reader.decompress(&bytes, &mut budget).unwrap().len(),
            MAX_RECORD_OUTPUT
        );
        assert_eq!(budget.output_left, MAX_RECORD_OUTPUT * 2);

        bytes.push(254);
        assert_eq!(
            reader.decompress(&bytes, &mut budget),
            Err(HuffError::Limit)
        );
        assert_eq!(budget.output_left, MAX_RECORD_OUTPUT * 2);
    }
}
