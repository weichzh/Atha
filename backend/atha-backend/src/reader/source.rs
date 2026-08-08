use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
};

use sha2::{Digest, Sha256};

pub const MAX_SOURCE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceError {
    InvalidSource,
    SourceTooLarge,
}

pub(super) struct SourceDigest {
    digest: Sha256,
    total: u64,
    max_bytes: u64,
}

impl SourceDigest {
    pub(super) fn new(identity_domain: &[u8], max_bytes: u64) -> Self {
        let mut digest = Sha256::new();
        digest.update(identity_domain);
        Self {
            digest,
            total: 0,
            max_bytes,
        }
    }

    pub(super) fn update(&mut self, bytes: &[u8]) -> Result<(), SourceError> {
        self.total = self
            .total
            .checked_add(bytes.len() as u64)
            .ok_or(SourceError::SourceTooLarge)?;
        if self.total > self.max_bytes {
            return Err(SourceError::SourceTooLarge);
        }
        self.digest.update(bytes);
        Ok(())
    }

    pub(super) fn finish(self) -> String {
        let mut value = String::with_capacity(64);
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for byte in self.digest.finalize() {
            value.push(char::from(HEX[usize::from(byte >> 4)]));
            value.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        value
    }
}

pub(super) fn fingerprint(
    source: &Path,
    identity_domain: &[u8],
    max_bytes: u64,
) -> Result<(String, File), SourceError> {
    fingerprint_with(source, identity_domain, max_bytes, |_, _| {})
}

pub(super) fn fingerprint_with(
    source: &Path,
    identity_domain: &[u8],
    max_bytes: u64,
    mut inspect: impl FnMut(&[u8], bool),
) -> Result<(String, File), SourceError> {
    let mut file = File::open(source).map_err(|_| SourceError::InvalidSource)?;
    let hash = hash_reader(&mut file, identity_domain, max_bytes, &mut inspect)?;
    file.seek(SeekFrom::Start(0))
        .map_err(|_| SourceError::InvalidSource)?;
    Ok((hash, file))
}

pub(super) fn hash_file(
    path: &Path,
    identity_domain: &[u8],
    max_bytes: u64,
) -> Result<String, SourceError> {
    let mut file = File::open(path).map_err(|_| SourceError::InvalidSource)?;
    hash_reader(&mut file, identity_domain, max_bytes, &mut |_, _| {})
}

fn hash_reader(
    reader: &mut (impl Read + Seek),
    identity_domain: &[u8],
    max_bytes: u64,
    inspect: &mut impl FnMut(&[u8], bool),
) -> Result<String, SourceError> {
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|_| SourceError::InvalidSource)?;
    let mut reader = BufReader::new(reader);
    let mut digest = SourceDigest::new(identity_domain, max_bytes);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| SourceError::InvalidSource)?;
        if read == 0 {
            break;
        }
        inspect(&buffer[..read], false);
        digest.update(&buffer[..read])?;
    }
    inspect(&[], true);
    Ok(digest.finish())
}
