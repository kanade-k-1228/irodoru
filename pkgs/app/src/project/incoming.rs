//! Native file-open requests, including launches forwarded to the existing process.
use anyhow::{Context, Result, ensure};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 256 * 1024;
const MAX_PATHS: usize = 64;

use super::is_project_path;

/// Keeps the project files among the positional command-line arguments.
pub fn argument_paths(paths: impl IntoIterator<Item = PathBuf>, cwd: &Path) -> Vec<PathBuf> {
    paths
        .into_iter()
        .filter(|path| !path.to_string_lossy().contains("://") && is_project_path(path))
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            }
        })
        .take(MAX_PATHS)
        .collect()
}

pub fn url_paths(urls: impl IntoIterator<Item = String>) -> Vec<PathBuf> {
    urls.into_iter()
        .filter(|value| value.len() as u64 <= MAX_BYTES)
        .filter_map(|value| url::Url::parse(&value).ok()?.to_file_path().ok())
        .filter(|path| is_project_path(path))
        .take(MAX_PATHS)
        .collect()
}

/// Atomic files survive the primary process starting up before its event loop is ready.
/// The directory lives beside the existing per-user instance lock, never in a shared temp dir.
#[derive(Clone)]
pub struct Inbox(PathBuf);
impl Inbox {
    pub fn new() -> Result<Self> {
        let dirs = directories::ProjectDirs::from("io.github", "kanade-k-1228", "irodoru")
            .context("設定フォルダが見つかりません")?;
        Self::at(dirs.config_dir().join("open-requests"))
    }

    fn at(path: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&path)?;
        Ok(Self(path))
    }

    pub fn forward(&self, paths: &[PathBuf]) -> Result<()> {
        ensure!(
            !paths.is_empty() && paths.len() <= MAX_PATHS,
            "開くファイル数が不正です"
        );
        ensure!(
            paths
                .iter()
                .all(|path| path.is_absolute() && is_project_path(path)),
            "開くパスが不正です"
        );
        let bytes = serde_json::to_vec(paths)?;
        ensure!(bytes.len() as u64 <= MAX_BYTES, "開くパスが長すぎます");
        let mut file = tempfile::NamedTempFile::new_in(&self.0)?;
        file.write_all(&bytes)?;
        file.as_file().sync_all()?;
        let order = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        file.persist(
            self.0
                .join(format!("{order:039}-{}.json", crate::domain::new_id())),
        )
        .map_err(|e| e.error)?;
        Ok(())
    }

    pub fn drain(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        let mut requests = std::fs::read_dir(&self.0)?.collect::<std::io::Result<Vec<_>>>()?;
        requests.sort_by_key(|entry| entry.file_name());
        for entry in requests.into_iter().take(MAX_PATHS) {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "json") || !entry.file_type()?.is_file() {
                continue;
            }
            // Remove consumed requests even if malformed, so they cannot poison later launches.
            let result = (|| -> Result<Vec<PathBuf>> {
                let mut bytes = Vec::new();
                std::fs::File::open(&path)?
                    .take(MAX_BYTES + 1)
                    .read_to_end(&mut bytes)?;
                ensure!(
                    bytes.len() as u64 <= MAX_BYTES,
                    "ファイルを開く要求が大きすぎます"
                );
                let batch: Vec<PathBuf> = serde_json::from_slice(&bytes)?;
                ensure!(batch.len() <= MAX_PATHS, "開くファイルが多すぎます");
                ensure!(
                    batch.iter().all(|p| p.is_absolute() && is_project_path(p)),
                    "開くパスが不正です"
                );
                Ok(batch)
            })();
            if let Ok(batch) = &result
                && paths.len() + batch.len() > MAX_PATHS
            {
                break;
            }
            std::fs::remove_file(path)?;
            match result {
                Ok(batch) => paths.extend(batch),
                Err(error) => eprintln!("プロジェクトを開く要求を読み込めません: {error:#}"),
            }
        }
        let mut seen = std::collections::HashSet::new();
        paths.retain(|path| seen.insert(path.canonicalize().unwrap_or_else(|_| path.clone())));
        Ok(paths)
    }
}
