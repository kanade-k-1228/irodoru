//! Model distribution data and validation, independent of storage and transport.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_ARCHIVE_SIZE: u64 = 10_000_000_000;
pub const REQUIRED: &[(&str, &[&str])] = &[
    (
        "anime",
        &[
            "LICENSES.txt",
            "provenance.json",
            "manifest.json",
            "tokenizer.json",
            "caption_tokenizer.json",
            "forced_splits.json",
            "backbone.onnx",
            "text_proj.onnx",
            "caption_proj.onnx",
            "speaker_encoder.onnx",
            "duration.onnx",
            "dit_context.onnx",
            "dit_step.onnx",
        ],
    ),
    (
        "codec",
        &[
            "LICENSES.txt",
            "provenance.json",
            "codec.json",
            "dacvae_encoder.onnx",
            "dacvae_decoder.onnx",
        ],
    ),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}
fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_filename(name: &str) -> bool {
    let edge = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
    name.bytes().next().is_some_and(edge)
        && name.bytes().last().is_some_and(edge)
        && name.bytes().all(|b| edge(b) || b == b'.')
}
fn valid_path(path: &str) -> bool {
    path.split_once('/')
        .is_some_and(|(folder, name)| matches!(folder, "anime" | "codec") && valid_filename(name))
}
pub fn validate_files(entries: &[Entry]) -> Result<BTreeSet<&str>> {
    let mut paths = BTreeSet::new();
    let mut total = 0u64;
    for entry in entries {
        anyhow::ensure!(valid_path(&entry.path), "Invalid distribution model path");
        anyhow::ensure!(
            paths.insert(entry.path.as_str()),
            "Duplicate model resource"
        );
        anyhow::ensure!(
            entry.size > 0 && entry.size <= MAX_ARCHIVE_SIZE,
            "Empty or invalid model file"
        );
        anyhow::ensure!(valid_sha256(&entry.sha256), "Invalid model file checksum");
        total = total
            .checked_add(entry.size)
            .context("model size overflow")?;
    }
    anyhow::ensure!(
        total <= MAX_ARCHIVE_SIZE,
        "Model files exceed archive limit"
    );
    for (folder, names) in REQUIRED {
        for name in *names {
            anyhow::ensure!(
                paths.contains(format!("{folder}/{name}").as_str()),
                "Incomplete model resources: {folder}/{name}"
            );
        }
    }
    Ok(paths)
}
pub fn valid_repository(repository: &str) -> bool {
    let parts: Vec<_> = repository.split('/').collect();
    let edge = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    repository.len() <= 255
        && parts.len() == 2
        && parts.iter().all(|part| {
            part.bytes().next().is_some_and(edge)
                && part.bytes().last().is_some_and(edge)
                && part.bytes().all(|b| edge(b) || b"._-".contains(&b))
        })
}

/// Required paths in the archive, in a stable order.
pub fn required_paths() -> impl Iterator<Item = String> {
    REQUIRED
        .iter()
        .flat_map(|(folder, names)| names.iter().map(move |name| format!("{folder}/{name}")))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Distribution {
    pub schema: u32,
    pub model: String,
    pub revision: Option<String>,
    pub repository: Option<String>,
    pub archive: Option<Archive>,
    pub files: Vec<Entry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Archive {
    pub format: String,
    pub asset: String,
    pub size: u64,
    pub sha256: String,
}

impl Distribution {
    pub fn is_unpublished(&self) -> bool {
        self.schema == 2
            && self.model == "anime"
            && self.revision.is_none()
            && self.repository.is_none()
            && self.archive.is_none()
            && self.files.is_empty()
    }

    /// Returns the total extracted size for a usable distribution, including an offline cache.
    pub fn validate(&self) -> Result<u64> {
        ensure!(
            self.schema == 2 && self.model == "anime",
            "invalid model distribution"
        );
        ensure!(
            self.revision.as_deref().is_some_and(valid_sha256),
            "invalid model revision"
        );
        ensure!(
            self.repository.as_deref().is_none_or(valid_repository),
            "invalid model repository"
        );
        let archive = self.archive.as_ref().context("missing model archive")?;
        ensure!(
            archive.format == "zip"
                && valid_filename(&archive.asset)
                && archive.size > 0
                && archive.size <= MAX_ARCHIVE_SIZE
                && valid_sha256(&archive.sha256),
            "invalid model archive"
        );
        validate_files(&self.files)?;
        // validate_files checks each size and the total before this sum.
        let total = self.files.iter().map(|entry| entry.size).sum();
        ensure!(total <= archive.size, "model files exceed archive size");
        Ok(total)
    }
}

#[cfg(test)]
mod tests;
