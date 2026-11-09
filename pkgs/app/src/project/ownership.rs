//! Prevent independent windows from writing the same archive, including path aliases.
use crate::project::Session;
use anyhow::{Context, Result, ensure};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
static PATHS: OnceLock<Mutex<HashMap<PathBuf, String>>> = OnceLock::new();
fn paths() -> &'static Mutex<HashMap<PathBuf, String>> {
    PATHS.get_or_init(Default::default)
}
#[derive(Default)]
pub(crate) struct WindowProject {
    id: String,
    path: Option<PathBuf>,
}
impl WindowProject {
    pub fn open(&mut self, path: &Path, create: bool) -> Result<Session> {
        let path = std::path::absolute(path)?;
        let path = if path.exists() {
            path.canonicalize()?
        } else {
            path.parent()
                .context("保存先が不正です")?
                .canonicalize()?
                .join(path.file_name().context("保存先が不正です")?)
        };
        if self.id.is_empty() {
            self.id = crate::domain::new_id();
        }
        let mut owners = paths()
            .lock()
            .map_err(|_| anyhow::anyhow!("project ownership lock poisoned"))?;
        ensure!(
            owners.get(&path).is_none_or(|owner| owner == &self.id),
            "このプロジェクトは別のウィンドウで開いています"
        );
        // Keep the previous ownership until loading succeeds, so failures never discard it.
        let session = if create {
            Session::create(path.clone())?
        } else {
            Session::open(path.clone())?
        };
        if let Some(old) = self.path.take() {
            owners.remove(&old);
        }
        owners.insert(path.clone(), self.id.clone());
        self.path = Some(path);
        Ok(session)
    }
}
impl Drop for WindowProject {
    fn drop(&mut self) {
        if let (Some(path), Ok(mut owners)) = (&self.path, paths().lock())
            && owners.get(path) == Some(&self.id)
        {
            owners.remove(path);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_reserve_paths_independently_and_release_on_close() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.iro");
        let b = dir.path().join("b.iro");
        let mut first = WindowProject::default();
        let mut second = WindowProject::default();
        first.open(&a, true).unwrap();
        assert!(second.open(&a, true).is_err());
        second.open(&b, true).unwrap();
        assert!(first.open(&b, true).is_err());
        assert!(first.open(&dir.path().join("missing.iro"), false).is_err());
        assert!(second.open(&a, true).is_err());
        drop(first);
        second.open(&a, true).unwrap();
        WindowProject::default().open(&b, true).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_parent_cannot_bypass_ownership() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let alias = dir.path().join("alias");
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        let mut first = WindowProject::default();
        first.open(&real.join("project.iro"), true).unwrap();
        assert!(
            WindowProject::default()
                .open(&alias.join("project.iro"), true)
                .is_err()
        );
    }
}
