//! Native file-open requests, including launches forwarded to the existing process.
#[cfg(feature = "desktop")]
use anyhow::Context;
use anyhow::{Result, ensure};
use std::{
    ffi::OsString,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 256 * 1024;
const MAX_PATHS: usize = 64;

fn is_project(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("iro") || ext.eq_ignore_ascii_case("zip"))
}

pub(crate) fn argument_paths(args: impl IntoIterator<Item = OsString>, cwd: &Path) -> Vec<PathBuf> {
    args.into_iter()
        .skip(1)
        .filter_map(|arg| {
            let text = arg.to_string_lossy();
            if text.starts_with('-') || text.contains("://") {
                return None;
            }
            let path = PathBuf::from(arg);
            is_project(&path).then(|| {
                if path.is_absolute() {
                    path
                } else {
                    cwd.join(path)
                }
            })
        })
        .take(MAX_PATHS)
        .collect()
}

pub(crate) fn url_paths(urls: impl IntoIterator<Item = String>) -> Vec<PathBuf> {
    urls.into_iter()
        .filter(|value| value.len() as u64 <= MAX_BYTES)
        .filter_map(|value| url::Url::parse(&value).ok()?.to_file_path().ok())
        .filter(|path| is_project(path))
        .take(MAX_PATHS)
        .collect()
}

/// Atomic files survive the primary process starting up before its event loop is ready.
/// The directory lives beside the existing per-user instance lock, never in a shared temp dir.
#[derive(Clone)]
pub(crate) struct Inbox(PathBuf);
impl Inbox {
    #[cfg(feature = "desktop")]
    pub(crate) fn new() -> Result<Self> {
        let dirs = directories::ProjectDirs::from("io.github", "kanade-k-1228", "irodoru")
            .context("設定フォルダが見つかりません")?;
        Self::at(dirs.config_dir().join("open-requests"))
    }

    fn at(path: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&path)?;
        Ok(Self(path))
    }

    pub(crate) fn forward(&self, paths: &[PathBuf]) -> Result<()> {
        ensure!(
            !paths.is_empty() && paths.len() <= MAX_PATHS,
            "開くファイル数が不正です"
        );
        ensure!(
            paths
                .iter()
                .all(|path| path.is_absolute() && is_project(path)),
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

    pub(crate) fn drain(&self) -> Result<Vec<PathBuf>> {
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
                    batch.iter().all(|p| p.is_absolute() && is_project(p)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_resolve_native_paths_and_ignore_flags_or_remote_urls() {
        let dir = tempfile::tempdir().unwrap();
        let args = [
            "irodoru",
            "--flag.iro",
            "https://host/file.iro",
            "file:///tmp/file.iro",
            "相対 パス.IRO",
            "legacy.zip",
            "other.txt",
        ];
        assert_eq!(
            argument_paths(args.map(OsString::from), dir.path()),
            vec![
                dir.path().join("相対 パス.IRO"),
                dir.path().join("legacy.zip")
            ]
        );
    }

    #[test]
    fn file_urls_decode_spaces_and_unicode_without_accepting_remote_projects() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("日本語 project.iro");
        let url = url::Url::from_file_path(&path).unwrap();
        assert_eq!(
            url_paths([
                url.into(),
                "https://host/file.iro".into(),
                "file:///tmp/other.txt".into()
            ]),
            vec![path]
        );
    }

    #[test]
    fn forwarded_requests_wait_for_startup_and_are_consumed_once() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = Inbox::at(dir.path().join("inbox")).unwrap();
        let first = dir.path().join("first.iro");
        let second = dir.path().join("second.zip");
        inbox
            .forward(&[first.clone(), first.clone(), second.clone()])
            .unwrap();
        assert_eq!(inbox.drain().unwrap(), vec![first, second]);
        assert!(inbox.drain().unwrap().is_empty());
    }

    #[test]
    fn each_poll_is_bounded_and_preserves_later_requests() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = Inbox::at(dir.path().join("inbox")).unwrap();
        let batch = (0..MAX_PATHS)
            .map(|i| dir.path().join(format!("{i}.iro")))
            .collect::<Vec<_>>();
        let last = dir.path().join("last.iro");
        inbox.forward(&batch).unwrap();
        inbox.forward(std::slice::from_ref(&last)).unwrap();
        assert_eq!(inbox.drain().unwrap(), batch);
        assert_eq!(inbox.drain().unwrap(), vec![last]);
        assert!(inbox.drain().unwrap().is_empty());
    }

    #[test]
    fn malformed_oversized_or_relative_requests_do_not_block_later_files() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = Inbox::at(dir.path().join("inbox")).unwrap();
        std::fs::write(inbox.0.join("a.json"), b"invalid").unwrap();
        std::fs::write(inbox.0.join("b.json"), vec![b' '; MAX_BYTES as usize + 1]).unwrap();
        std::fs::write(inbox.0.join("c.json"), br#"["relative.iro"]"#).unwrap();
        let path = dir.path().join("valid.iro");
        inbox.forward(std::slice::from_ref(&path)).unwrap();
        assert_eq!(inbox.drain().unwrap(), vec![path]);
        assert_eq!(std::fs::read_dir(&inbox.0).unwrap().count(), 0);
        assert!(inbox.forward(&[PathBuf::from("relative.iro")]).is_err());
    }
}
