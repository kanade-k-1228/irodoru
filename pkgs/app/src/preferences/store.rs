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
            .and_then(|p| confy::load_path(p).ok())
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

/// One application-wide writer: theme changes must not wait behind a window's inference queue.
pub struct SettingsWriter {
    sender: Option<std::sync::mpsc::Sender<Theme>>,
    worker: Option<std::thread::JoinHandle<()>>,
    pub errors: std::sync::mpsc::Receiver<String>,
}
impl SettingsWriter {
    pub fn start() -> Result<Self> {
        Self::with_save(|theme| Settings { theme }.save())
    }
    fn with_save(mut save: impl FnMut(Theme) -> Result<()> + Send + 'static) -> Result<Self> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let (report, errors) = std::sync::mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("irodoru-settings".into())
            .spawn(move || {
                for theme in receiver {
                    if let Err(error) = save(theme) {
                        let _ = report.send(format!("テーマを保存できませんでした: {error:#}"));
                    }
                }
            })?;
        Ok(Self {
            sender: Some(sender),
            worker: Some(worker),
            errors,
        })
    }
    pub fn save(&self, theme: Theme) -> Result<()> {
        self.sender
            .as_ref()
            .context("設定保存が終了しました")?
            .send(theme)
            .context("設定保存が終了しました")
    }
}
impl Drop for SettingsWriter {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
