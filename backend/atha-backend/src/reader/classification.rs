use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::is_reparse_point;

pub(super) const MAX_LABELS: usize = 32;
const MAX_LABEL_CHARS: usize = 128;
pub(super) const MAX_LABEL_BYTES: usize = MAX_LABEL_CHARS * 4;
pub(crate) const MAX_ORGANIZATION_BYTES: u64 = 128 * 1024;
const STAGING_PREFIX: &str = ".organization-staging-";
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

pub(crate) const BOOK_ORGANIZATION_DIR: &str = "BookOrganization";

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BookOrganization {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorite: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjects: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<Vec<String>>,
}

impl BookOrganization {
    pub(super) fn normalized(self) -> Result<Self, ()> {
        Ok(Self {
            favorite: self.favorite,
            tags: self.tags.map(normalize_labels).transpose()?,
            subjects: self.subjects.map(normalize_labels).transpose()?,
            series: self.series.map(normalize_labels).transpose()?,
        })
    }

    pub(super) fn is_valid(&self) -> bool {
        [&self.tags, &self.subjects, &self.series]
            .into_iter()
            .all(|values| values.as_deref().is_none_or(valid_labels))
    }

    pub(super) fn update(&mut self, delta: Self) {
        self.favorite = delta.favorite.or(self.favorite);
        for (stored, update) in [
            (&mut self.tags, delta.tags),
            (&mut self.subjects, delta.subjects),
            (&mut self.series, delta.series),
        ] {
            if update.is_some() {
                *stored = update;
            }
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceClassification {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) subjects: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) series: Vec<String>,
}

impl SourceClassification {
    pub(super) fn is_empty(&self) -> bool {
        self.tags.is_empty() && self.subjects.is_empty() && self.series.is_empty()
    }

    pub(super) fn is_valid(&self) -> bool {
        valid_labels(&self.tags) && valid_labels(&self.subjects) && valid_labels(&self.series)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceProjection {
    schema: u8,
    content_version: String,
    classification: SourceClassification,
}

pub(super) fn cache_path(imports: &Path, id: &str) -> std::path::PathBuf {
    imports.join(format!(".atha-classification-{id}.json"))
}

pub(super) fn read_cache(imports: &Path, id: &str) -> Option<SourceClassification> {
    if !fs::symlink_metadata(imports)
        .is_ok_and(|entry| entry.file_type().is_dir() && !is_reparse_point(&entry))
    {
        return None;
    }
    let projection: SourceProjection = read_json(&cache_path(imports, id)).ok()??;
    (projection.schema == 1
        && projection.content_version == id
        && projection.classification.is_valid())
    .then_some(projection.classification)
}

pub(super) fn write_cache(
    imports: &Path,
    id: &str,
    classification: SourceClassification,
) -> io::Result<()> {
    write_json(
        &cache_path(imports, id),
        &SourceProjection {
            schema: 1,
            content_version: id.to_owned(),
            classification,
        },
    )
}

pub(super) fn read_json<T: DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_file()
        || is_reparse_point(&metadata)
        || metadata.len() > MAX_ORGANIZATION_BYTES
    {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_ORGANIZATION_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != metadata.len() {
        return Err(io::ErrorKind::InvalidData.into());
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(super) fn write_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let sequence = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
    let temporary =
        path.with_file_name(format!("{STAGING_PREFIX}{}-{sequence}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        serde_json::to_writer(&mut file, value)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

pub(super) fn is_temporary_file(name: &str) -> bool {
    name.strip_prefix(STAGING_PREFIX)
        .and_then(|suffix| suffix.split_once('-'))
        .is_some_and(|(pid, sequence)| {
            [pid, sequence]
                .into_iter()
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        })
}

pub(super) fn add_label(labels: &mut Vec<String>, raw: &str) {
    if labels.len() < MAX_LABELS
        && let Ok(Some(label)) = normalize_label(raw)
        && !labels.contains(&label)
    {
        labels.push(label);
    }
}

fn normalize_labels(values: Vec<String>) -> Result<Vec<String>, ()> {
    if values.len() > MAX_LABELS {
        return Err(());
    }
    let mut labels = Vec::with_capacity(values.len());
    for value in values {
        if let Some(label) = normalize_label(&value)?
            && !labels.contains(&label)
        {
            labels.push(label);
        }
    }
    Ok(labels)
}

fn valid_labels(labels: &[String]) -> bool {
    labels.len() <= MAX_LABELS
        && labels.iter().enumerate().all(|(index, label)| {
            normalize_label(label).is_ok_and(|value| value.as_ref() == Some(label))
                && !labels[..index].contains(label)
        })
}

pub(super) fn normalize_label(raw: &str) -> Result<Option<String>, ()> {
    if raw.len() > MAX_LABEL_BYTES || raw.chars().any(|ch| ch.is_control() && !ch.is_whitespace()) {
        return Err(());
    }
    let label = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if label.chars().count() > MAX_LABEL_CHARS {
        return Err(());
    }
    Ok((!label.is_empty()).then_some(label))
}

pub(super) fn append_source_label_text(target: &mut Option<String>, text: Option<&str>) {
    if let (Some(value), Some(text)) = (target.as_mut(), text)
        && value.len().saturating_add(text.len()) <= MAX_LABEL_BYTES
    {
        value.push_str(text);
    } else {
        *target = None;
    }
}
