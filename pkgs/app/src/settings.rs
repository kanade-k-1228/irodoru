use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
#[derive(Default, Serialize, Deserialize)]
pub struct Settings {
    pub light: bool,
}
impl Settings {
    fn path() -> Result<std::path::PathBuf> {
        Ok(
            directories::ProjectDirs::from("io.github", "kanade-k-1228", "irodoru")
                .context("設定フォルダが見つかりません")?
                .config_dir()
                .join("settings.json"),
        )
    }
    pub fn load() -> Self {
        Self::path()
            .ok()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|v| serde_json::from_slice(&v).ok())
            .unwrap_or_default()
    }
    pub fn save(&self) -> Result<()> {
        let p = Self::path()?;
        std::fs::create_dir_all(p.parent().unwrap())?;
        crate::library::atomic_bytes(&p, &serde_json::to_vec(self)?)?;
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
    sender: Option<std::sync::mpsc::Sender<bool>>,
    worker: Option<std::thread::JoinHandle<()>>,
    pub errors: std::sync::mpsc::Receiver<String>,
}
impl SettingsWriter {
    pub fn start() -> Result<Self> {
        Self::with_save(|light| Settings { light }.save())
    }
    fn with_save(mut save: impl FnMut(bool) -> Result<()> + Send + 'static) -> Result<Self> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let (report, errors) = std::sync::mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("irodoru-settings".into())
            .spawn(move || {
                for light in receiver {
                    if let Err(error) = save(light) {
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
    pub fn save(&self, light: bool) -> Result<()> {
        self.sender
            .as_ref()
            .context("設定保存が終了しました")?
            .send(light)
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn theme_writer_preserves_order_recovers_and_drains_on_exit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let saved = path.clone();
        let mut first = true;
        let writer = SettingsWriter::with_save(move |light| {
            if first {
                first = false;
                anyhow::bail!("temporary write error");
            }
            crate::library::atomic_bytes(&saved, &serde_json::to_vec(&Settings { light })?)?;
            Ok(())
        })
        .unwrap();
        writer.save(true).unwrap();
        writer.save(true).unwrap();
        writer.save(false).unwrap();
        assert!(
            writer
                .errors
                .recv()
                .unwrap()
                .contains("temporary write error")
        );
        drop(writer);
        assert!(
            !serde_json::from_slice::<Settings>(&std::fs::read(path).unwrap())
                .unwrap()
                .light
        );
    }
}
