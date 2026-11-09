//! Model source preferences are separate from the independently saved theme.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Default, Serialize, Deserialize)]
pub struct ModelSettings {
    pub directory: Option<PathBuf>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_source_round_trip_and_reset_preserve_theme() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("models.yaml");
        let theme = dir.path().join("config.yaml");
        std::fs::write(&theme, "theme: light\n").unwrap();
        assert!(ModelSettings::load_from(&path).unwrap().directory.is_none());
        let directory = dir.path().join("日本語のモデル");
        ModelSettings {
            directory: Some(directory.clone()),
        }
        .save_to(&path)
        .unwrap();
        assert_eq!(
            ModelSettings::load_from(&path).unwrap().directory,
            Some(directory)
        );
        ModelSettings::default().save_to(&path).unwrap();
        assert!(ModelSettings::load_from(&path).unwrap().directory.is_none());
        assert_eq!(std::fs::read_to_string(theme).unwrap(), "theme: light\n");
    }
}
