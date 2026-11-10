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
