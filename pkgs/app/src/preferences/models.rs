//! Model source preferences are separate from the independently saved theme.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSettings {
    pub directory: Option<PathBuf>,
    /// Display name only; the source ZIP is not needed after import.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_name: Option<String>,
}

impl ModelSettings {
    fn path() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("io.github", "kanade-k-1228", "irodoru")
            .context("設定フォルダが見つかりません")?;
        Ok(dirs.config_dir().join("models.yaml"))
    }

    pub fn load() -> Result<Self> {
        Self::load_from(&Self::path()?)
    }

    fn load_from(path: &Path) -> Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => serde_yaml::from_slice(&bytes).context("モデル設定を読み込めません"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error).context("モデル設定を読み込めません"),
        }
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path()?)
    }

    fn save_to(&self, path: &Path) -> Result<()> {
        std::fs::create_dir_all(path.parent().context("設定の保存先が不正です")?)?;
        crate::files::atomic_bytes(path, serde_yaml::to_string(self)?.as_bytes())?;
        Ok(())
    }
}
