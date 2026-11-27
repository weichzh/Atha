//! Adapt bounded HUFF records to the uncompressed input supported by boko's Book.

use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    ops::Range,
    path::Path,
    sync::Arc,
};

use boko::{
    ByteSource, FileSource, Format, Importer,
    import::{Azw3Importer, MobiImporter},
};

use super::{ImportError, MAX_RESOURCE_BYTES, MAX_TEXT_BYTES, be_u16, be_u32, structure};
use crate::reader::huffcdic::{
    Budget, HuffCdicReader, HuffError, MAX_CACHE_BYTES, strip_trailing_data,
};

// These are independent limits for this importer, not estimates from source size.
const MAX_PACKED_RECORD_BYTES: u64 = 1024 * 1024;
const MAX_CODEBOOK_BYTES: u64 = 16 * 1024 * 1024;
const MAX_HUFF_RECORDS: usize = 512;
const MAX_HUFF_ENTRIES: usize = 262_144;
const MAX_CODES: usize = 128 * 1024 * 1024;
const PDB_TABLE_START: u64 = 78;

struct RawSource(FileSource);

impl RawSource {
    fn check_read(&self, offset: u64, len: usize) -> io::Result<()> {
        if len as u64 > MAX_RESOURCE_BYTES
            || offset
                .checked_add(len as u64)
                .is_none_or(|end| end > self.len())
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(())
    }
}

impl ByteSource for RawSource {
    fn len(&self) -> u64 {
        self.0.len()
    }

    fn read_at_into(&self, offset: u64, buffer: &mut [u8]) -> io::Result<usize> {
        self.check_read(offset, buffer.len())?;
        self.0.read_at_into(offset, buffer)
    }

    fn read_at(&self, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        self.check_read(offset, len)?;
        self.0.read_at(offset, len)
    }
}

pub(super) fn open_raw(path: &Path, format: Format) -> Result<Box<dyn Importer>, ImportError> {
    let source = File::open(path)
        .and_then(FileSource::new)
        .map_err(|_| ImportError::InvalidSource)?;
    let source = Arc::new(RawSource(source));
    let importer: Box<dyn Importer> = match format {
        Format::Azw3 => {
            Box::new(Azw3Importer::from_source(source).map_err(|_| ImportError::InvalidStructure)?)
        }
        Format::Mobi => {
            Box::new(MobiImporter::from_source(source).map_err(|_| ImportError::InvalidStructure)?)
        }
        _ => return Err(ImportError::Unsupported),
    };
    Ok(importer)
}

pub(super) struct Plan {
    pub offsets: Vec<u64>, // validated PDB offsets plus EOF
    pub table: Vec<u8>,
    pub record_zero: Vec<u8>,
    pub text_records: usize,
    pub codebook: Range<usize>,
    pub is_kf8: bool,
}

pub(super) struct ImageRecords {
    offsets: Vec<u64>,
    first_image: usize,
    text_records: usize,
}

impl ImageRecords {
    pub(super) fn bytes(&self, asset: &str) -> Result<u64, ImportError> {
        // boko 0.5 lists images by record index relative to first_image.
        let index = asset
            .strip_prefix("images/image_")
            .and_then(|name| name.split_once('.'))
            .and_then(|(number, _)| number.parse::<usize>().ok())
            .and_then(|relative| self.first_image.checked_add(relative))
            .filter(|index| *index > self.text_records && *index < self.offsets.len() - 1)
            .ok_or(ImportError::InvalidImage)?;
        // Normalization keeps all non-text record lengths unchanged.
        Ok(self.offsets[index + 1] - self.offsets[index])
    }
}

impl Plan {
    pub(super) fn normalize(
        mut self,
        snapshot: &Path,
        target: &Path,
    ) -> Result<ImageRecords, ImportError> {
        // boko uses the minimum version field for its KF8 dispatch. Do not
        // hand it a candidate that could switch to an unvalidated combo stream.
        if self.is_kf8 && be_u32(&self.record_zero, 0x68)? != 8 {
            return Err(ImportError::InvalidStructure);
        }
        if self.codebook.start <= self.text_records {
            return Err(ImportError::InvalidStructure);
        }
        if self.codebook.len() > MAX_HUFF_RECORDS
            || self.offsets[self.codebook.end] - self.offsets[self.codebook.start]
                > MAX_CODEBOOK_BYTES
        {
            return Err(ImportError::TextTooLarge);
        }

        let mut input = File::open(snapshot).map_err(|_| ImportError::InvalidSource)?;
        let huff = self.read_packed(&mut input, self.codebook.start)?;
        let cdics = (self.codebook.start + 1..self.codebook.end)
            .map(|index| self.read_packed(&mut input, index))
            .collect::<Result<Vec<_>, _>>()?;
        let cdic_refs = cdics.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let mut reader = HuffCdicReader::new(&huff, &cdic_refs, MAX_HUFF_ENTRIES, MAX_CACHE_BYTES)
            .map_err(decode_error)?;

        let mut output = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(target)
            .map_err(|_| ImportError::WriteFailed)?;
        let mut output_offset = self.offsets[0];
        copy_range(&mut input, &mut output, 0..output_offset)?;
        self.record_zero[..2].copy_from_slice(&1_u16.to_be_bytes());
        let flags = if be_u32(&self.record_zero, 20)? >= 0xe4 {
            let flags = be_u16(&self.record_zero, 0xf2)?;
            self.record_zero[0xf2..0xf4].fill(0);
            flags
        } else {
            0
        };
        output
            .write_all(&self.record_zero)
            .map_err(|_| ImportError::WriteFailed)?;
        output_offset += self.record_zero.len() as u64;

        let mut budget = Budget {
            output_left: MAX_TEXT_BYTES as usize,
            codes_left: Some(MAX_CODES),
        };
        for index in 1..=self.text_records {
            let packed = self.read_packed(&mut input, index)?;
            let decoded = reader
                .decompress(strip_trailing_data(&packed, flags), &mut budget)
                .map_err(decode_error)?;
            set_offset(&mut self.table, index, output_offset)?;
            output
                .write_all(&decoded)
                .map_err(|_| ImportError::WriteFailed)?;
            output_offset += decoded.len() as u64;
        }
        let actual_text_bytes = MAX_TEXT_BYTES as usize - budget.output_left;
        if actual_text_bytes == 0 {
            return Err(ImportError::InvalidStructure);
        }
        let layout = structure::validate(
            &mut input,
            &self.offsets,
            self.text_records,
            &self.record_zero,
            self.is_kf8,
            actual_text_bytes,
        )?;
        layout.verify_projection(
            &mut output,
            self.offsets[1],
            actual_text_bytes,
            self.record_zero.len(),
        )?;
        output
            .seek(SeekFrom::Start(output_offset))
            .map_err(|_| ImportError::WriteFailed)?;

        // Only the byte offsets change. Copy the non-text range without loading
        // its images or changing any record IDs used by indices and assets.
        let tail = self.offsets[self.text_records + 1];
        let eof = self.offsets[self.offsets.len() - 1];
        for (index, &offset) in self.offsets.iter().enumerate().skip(self.text_records + 1) {
            if index + 1 == self.offsets.len() {
                break;
            }
            set_offset(&mut self.table, index, output_offset + (offset - tail))?;
        }
        copy_range(&mut input, &mut output, tail..eof)?;
        output
            .seek(SeekFrom::Start(PDB_TABLE_START))
            .and_then(|_| output.write_all(&self.table))
            .map_err(|_| ImportError::WriteFailed)?;
        Ok(ImageRecords {
            offsets: self.offsets,
            first_image: be_u32(&self.record_zero, 0x6c)? as usize,
            text_records: self.text_records,
        })
    }

    fn read_packed(&self, file: &mut File, index: usize) -> Result<Vec<u8>, ImportError> {
        let start = self.offsets[index];
        let length = self.offsets[index + 1] - start;
        if length > MAX_PACKED_RECORD_BYTES {
            return Err(ImportError::TextTooLarge);
        }
        let mut bytes = vec![0; length as usize];
        file.seek(SeekFrom::Start(start))
            .and_then(|_| file.read_exact(&mut bytes))
            .map_err(|_| ImportError::InvalidStructure)?;
        Ok(bytes)
    }
}

fn set_offset(table: &mut [u8], index: usize, offset: u64) -> Result<(), ImportError> {
    let offset = u32::try_from(offset).map_err(|_| ImportError::TextTooLarge)?;
    table[index * 8..index * 8 + 4].copy_from_slice(&offset.to_be_bytes());
    Ok(())
}

fn copy_range(input: &mut File, output: &mut File, range: Range<u64>) -> Result<(), ImportError> {
    let length = range.end - range.start;
    input
        .seek(SeekFrom::Start(range.start))
        .map_err(|_| ImportError::InvalidStructure)?;
    let written =
        io::copy(&mut input.take(length), output).map_err(|_| ImportError::WriteFailed)?;
    if written != length {
        return Err(ImportError::InvalidStructure);
    }
    Ok(())
}

fn decode_error(error: HuffError) -> ImportError {
    match error {
        HuffError::InvalidData => ImportError::InvalidStructure,
        HuffError::Limit => ImportError::TextTooLarge,
    }
}
