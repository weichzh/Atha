use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use zip::ZipArchive;

use super::source::{self, SourceError};

pub use super::source::MAX_SOURCE_BYTES;
pub(super) const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
pub(super) const MAX_MEMBER_BYTES: u64 = 16 * 1024 * 1024;
pub(super) const MAX_ENTRIES: usize = 10_000;

pub(super) type Archive = ZipArchive<File>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ArchiveError {
    InvalidSource,
    SourceTooLarge,
    InvalidArchive,
    ArchiveTooLarge,
    UnsafePath,
    Encrypted,
    WriteFailed,
}

#[derive(Debug)]
pub(super) struct ArchiveIndex {
    members: HashMap<String, usize>,
}

pub(super) fn fingerprint(source: &Path) -> Result<(String, File), ArchiveError> {
    let (hash, mut file) =
        source::fingerprint(source, b"", MAX_SOURCE_BYTES).map_err(ArchiveError::from)?;
    verify_entry_count(&mut file)?;
    Ok((hash, file))
}

pub(super) fn open_fingerprinted(file: File) -> Result<Archive, ArchiveError> {
    ZipArchive::new(file).map_err(|_| ArchiveError::InvalidArchive)
}

fn verify_entry_count(reader: &mut (impl Read + Seek)) -> Result<(), ArchiveError> {
    // ponytail: zip 8.6 has no pre-allocation entry cap. This bounds its standard terminal
    // EOCD hint; replace it when zip::read::Config gains a max-entries option.
    const EOCD_BYTES: u64 = 22;
    const MAX_COMMENT_BYTES: u64 = u16::MAX as u64;
    let length = reader
        .seek(SeekFrom::End(0))
        .map_err(|_| ArchiveError::InvalidArchive)?;
    let window = length.min(EOCD_BYTES + MAX_COMMENT_BYTES);
    reader
        .seek(SeekFrom::End(
            -i64::try_from(window).map_err(|_| ArchiveError::InvalidArchive)?,
        ))
        .map_err(|_| ArchiveError::InvalidArchive)?;
    let mut tail = vec![0; usize::try_from(window).map_err(|_| ArchiveError::InvalidArchive)?];
    reader
        .read_exact(&mut tail)
        .map_err(|_| ArchiveError::InvalidArchive)?;
    if tail.len() < EOCD_BYTES as usize {
        return Err(ArchiveError::InvalidArchive);
    }
    let mut candidates = tail.windows(4).enumerate().filter_map(|(offset, bytes)| {
        if bytes != b"PK\x05\x06" || offset + EOCD_BYTES as usize > tail.len() {
            return None;
        }
        let comment = usize::from(u16::from_le_bytes([tail[offset + 20], tail[offset + 21]]));
        (offset + EOCD_BYTES as usize + comment == tail.len()).then_some(offset)
    });
    let offset = candidates.next_back().ok_or(ArchiveError::InvalidArchive)?;
    if candidates.next_back().is_some() {
        return Err(ArchiveError::InvalidArchive);
    }
    let disk = u16::from_le_bytes([tail[offset + 4], tail[offset + 5]]);
    let directory_disk = u16::from_le_bytes([tail[offset + 6], tail[offset + 7]]);
    let disk_entries = u16::from_le_bytes([tail[offset + 8], tail[offset + 9]]);
    let entries = u16::from_le_bytes([tail[offset + 10], tail[offset + 11]]);
    let directory_size = u64::from(u32::from_le_bytes(
        tail[offset + 12..offset + 16]
            .try_into()
            .map_err(|_| ArchiveError::InvalidArchive)?,
    ));
    let directory_offset = u64::from(u32::from_le_bytes(
        tail[offset + 16..offset + 20]
            .try_into()
            .map_err(|_| ArchiveError::InvalidArchive)?,
    ));
    if disk != 0
        || directory_disk != 0
        || disk_entries != entries
        || usize::from(entries) > MAX_ENTRIES
    {
        return Err(ArchiveError::InvalidArchive);
    }
    let eocd_offset = length - window + offset as u64;
    let minimum_directory_size = u64::from(entries) * 46;
    if directory_size < minimum_directory_size || directory_size > eocd_offset {
        return Err(ArchiveError::InvalidArchive);
    }
    let directory_start = eocd_offset - directory_size;
    if directory_offset > directory_start {
        return Err(ArchiveError::InvalidArchive);
    }
    if entries != 0 {
        reader
            .seek(SeekFrom::Start(directory_start))
            .map_err(|_| ArchiveError::InvalidArchive)?;
        let mut signature = [0; 4];
        reader
            .read_exact(&mut signature)
            .map_err(|_| ArchiveError::InvalidArchive)?;
        if signature != *b"PK\x01\x02" {
            return Err(ArchiveError::InvalidArchive);
        }
    }
    Ok(())
}

pub(super) fn inspect(archive: &mut Archive) -> Result<ArchiveIndex, ArchiveError> {
    if archive.is_empty()
        || archive.len() > MAX_ENTRIES
        || archive
            .has_overlapping_files()
            .map_err(|_| ArchiveError::InvalidArchive)?
    {
        return Err(ArchiveError::InvalidArchive);
    }
    let mut members = HashMap::with_capacity(archive.len());
    let mut folded = HashSet::with_capacity(archive.len());
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index_raw(index)
            .map_err(|_| ArchiveError::InvalidArchive)?;
        let name = safe_path(entry.name().trim_end_matches('/'))?;
        if entry.encrypted() {
            return Err(ArchiveError::Encrypted);
        }
        if entry.is_symlink() {
            return Err(ArchiveError::UnsafePath);
        }
        total = total
            .checked_add(entry.size())
            .ok_or(ArchiveError::ArchiveTooLarge)?;
        if entry.size() > MAX_MEMBER_BYTES || total > MAX_TOTAL_BYTES {
            return Err(ArchiveError::ArchiveTooLarge);
        }
        if !folded.insert(name.to_lowercase()) {
            return Err(ArchiveError::UnsafePath);
        }
        if entry.is_file() {
            members.insert(name, index);
        }
    }
    Ok(ArchiveIndex { members })
}

impl ArchiveIndex {
    pub(super) fn contains(&self, path: &str) -> bool {
        self.members.contains_key(path)
    }

    pub(super) fn paths(&self) -> impl Iterator<Item = &str> {
        self.members.keys().map(String::as_str)
    }

    fn require(&self, path: &str) -> Result<usize, ArchiveError> {
        self.members
            .get(path)
            .copied()
            .ok_or(ArchiveError::InvalidArchive)
    }
}

pub(super) fn require(index: &ArchiveIndex, path: &str) -> Result<(), ArchiveError> {
    index.require(path).map(|_| ())
}

pub(super) fn read(
    archive: &mut Archive,
    index: &ArchiveIndex,
    path: &str,
) -> Result<Vec<u8>, ArchiveError> {
    let entry = archive
        .by_index(index.require(path)?)
        .map_err(|_| ArchiveError::InvalidArchive)?;
    let mut bytes = Vec::with_capacity(usize::try_from(entry.size()).unwrap_or(0));
    entry
        .take(MAX_MEMBER_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ArchiveError::InvalidArchive)?;
    if bytes.len() as u64 > MAX_MEMBER_BYTES {
        return Err(ArchiveError::ArchiveTooLarge);
    }
    Ok(bytes)
}

pub(super) fn copy(
    archive: &mut Archive,
    index: &ArchiveIndex,
    path: &str,
    staging: &Path,
    extracted: &mut u64,
) -> Result<(), ArchiveError> {
    let entry = archive
        .by_index(index.require(path)?)
        .map_err(|_| ArchiveError::InvalidArchive)?;
    let destination = staging.join(path);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|_| ArchiveError::WriteFailed)?;
    }
    let mut output = File::create(destination).map_err(|_| ArchiveError::WriteFailed)?;
    let copied = std::io::copy(&mut entry.take(MAX_MEMBER_BYTES + 1), &mut output)
        .map_err(|_| ArchiveError::InvalidArchive)?;
    if copied > MAX_MEMBER_BYTES {
        return Err(ArchiveError::ArchiveTooLarge);
    }
    add_extracted(extracted, copied)?;
    Ok(())
}

pub(super) fn add_extracted(total: &mut u64, bytes: u64) -> Result<(), ArchiveError> {
    *total = total
        .checked_add(bytes)
        .filter(|total| *total <= MAX_TOTAL_BYTES)
        .ok_or(ArchiveError::ArchiveTooLarge)?;
    Ok(())
}

pub(super) fn safe_path(value: &str) -> Result<String, ArchiveError> {
    if value.is_empty()
        || value.len() > 512
        || value.starts_with('/')
        || value.contains(['\0', '\\', ':', '%', '?', '#'])
        || value.chars().any(char::is_control)
    {
        return Err(ArchiveError::UnsafePath);
    }
    for part in value.split('/') {
        let stem = part.split('.').next().unwrap_or_default();
        let upper_stem = stem.to_ascii_uppercase();
        let reserved = matches!(upper_stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || upper_stem.len() == 4
                && (upper_stem.starts_with("COM") || upper_stem.starts_with("LPT"))
                && upper_stem.as_bytes()[3].is_ascii_digit()
                && upper_stem.as_bytes()[3] != b'0';
        if part.is_empty() || matches!(part, "." | "..") || part.ends_with(['.', ' ']) || reserved {
            return Err(ArchiveError::UnsafePath);
        }
    }
    Ok(value.to_owned())
}

pub(super) fn hash_file(path: &Path) -> Result<String, ArchiveError> {
    source::hash_file(path, b"", MAX_SOURCE_BYTES).map_err(ArchiveError::from)
}

impl From<SourceError> for ArchiveError {
    fn from(error: SourceError) -> Self {
        match error {
            SourceError::InvalidSource => Self::InvalidSource,
            SourceError::SourceTooLarge => Self::SourceTooLarge,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn actual_extracted_bytes_cannot_exceed_the_archive_budget() {
        let mut total = MAX_TOTAL_BYTES;
        assert_eq!(
            add_extracted(&mut total, 1),
            Err(ArchiveError::ArchiveTooLarge)
        );
    }

    #[test]
    fn terminal_entry_hint_is_bounded_before_zip_metadata_is_allocated() {
        assert_eq!(
            verify_entry_count(&mut Cursor::new(eocd_with_directory(MAX_ENTRIES as u16,))),
            Ok(())
        );
        assert_eq!(
            verify_entry_count(&mut Cursor::new(eocd(MAX_ENTRIES as u16 + 1, &[]))),
            Err(ArchiveError::InvalidArchive)
        );
        let fake_small_eocd = eocd(0, &[]);
        assert_eq!(
            verify_entry_count(&mut Cursor::new(eocd(
                MAX_ENTRIES as u16 + 1,
                &fake_small_eocd,
            ))),
            Err(ArchiveError::InvalidArchive)
        );

        let mut comment = b"ordinary comment containing PK\x05\x06 bytes".to_vec();
        let archive = eocd(0, &comment);
        assert_eq!(verify_entry_count(&mut Cursor::new(archive)), Ok(()));
        comment.extend_from_slice(&fake_small_eocd);
        assert_eq!(
            verify_entry_count(&mut Cursor::new(eocd(0, &comment))),
            Err(ArchiveError::InvalidArchive)
        );
    }

    fn eocd(entries: u16, comment: &[u8]) -> Vec<u8> {
        let mut value = b"PK\x05\x06\0\0\0\0".to_vec();
        value.extend_from_slice(&entries.to_le_bytes());
        value.extend_from_slice(&entries.to_le_bytes());
        value.extend_from_slice(&[0; 8]);
        value.extend_from_slice(
            &u16::try_from(comment.len())
                .expect("test ZIP comment")
                .to_le_bytes(),
        );
        value.extend_from_slice(comment);
        value
    }

    fn eocd_with_directory(entries: u16) -> Vec<u8> {
        let directory_size = usize::from(entries) * 46;
        let mut value = vec![0; directory_size];
        value[..4].copy_from_slice(b"PK\x01\x02");
        let mut footer = eocd(entries, &[]);
        footer[12..16].copy_from_slice(
            &u32::try_from(directory_size)
                .expect("test central directory size")
                .to_le_bytes(),
        );
        value.extend_from_slice(&footer);
        value
    }
}
