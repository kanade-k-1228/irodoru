use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    #[default]
    Dark,
}
impl Theme {
    pub fn is_light(self) -> bool {
        self == Self::Light
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Settings {
    pub theme: Theme,
}

impl Settings {
    fn path() -> Result<std::path::PathBuf> {
        Ok(crate::files::project_dirs()
            .context("設定フォルダが見つかりません")?
            .config_dir()
            .join("config.yaml"))
    }
    pub fn load() -> Self {
        Self::path()
            .ok()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_yaml::from_slice(&bytes).ok())
            .unwrap_or_default()
    }
    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path()?)
    }
    fn save_to(&self, path: &std::path::Path) -> Result<()> {
        let parent = path.parent().context("設定の保存先が不正です")?;
        std::fs::create_dir_all(parent)?;
        crate::files::atomic_bytes(path, serde_yaml::to_string(self)?.as_bytes())?;
        Ok(())
    }
}

/// Prevent two app processes from independently overwriting the same project ZIP.
pub fn lock_instance() -> Result<std::fs::File> {
    let path = Settings::path()?.with_file_name("instance.lock");
    std::fs::create_dir_all(path.parent().unwrap())?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock()
        .context("irodoruは既に起動しています。開いているウィンドウを使用してください。")?;
    Ok(file)
}
