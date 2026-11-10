use super::archive::extract;
use crate::{
    domain::{AudioSource, new_id},
    library::Library,
};
use anyhow::{Result, bail};
use serde::Serialize;
use std::{
    fs::File,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SaveState {
    #[default]
    Closed,
    Saving,
    Saved,
    Error,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ProjectStatus {
    pub id: Option<String>,
    pub name: Option<String>,
    pub path: Option<String>,
    pub state: SaveState,
    pub error: Option<String>,
}

pub struct Session {
    pub id: String,
    pub library: Library,
    pub previews: Library,
    pub path: PathBuf,
    pub(super) revision: u64,
    pub(super) saved: u64,
    pub(super) error: Option<String>,
    // Own the working files until the session and its snapshots have gone away.
    _directory: tempfile::TempDir,
}
impl Session {
    pub fn create(path: PathBuf) -> Result<Self> {
        super::validate_path(&path)?;
        let directory = tempfile::Builder::new()
            .prefix("irodoru-project-")
            .tempdir()?;
        let library = Library::open(directory.path())?;
        library.save_editor(&Default::default())?;
        let previews = Library::open(&directory.path().join(".previews"))?;
        Ok(Self {
            id: new_id(),
            library,
            previews,
            path,
            revision: 1,
            saved: 0,
            error: None,
            _directory: directory,
        })
    }
    pub fn open(path: PathBuf) -> Result<Self> {
        let session = Self::create(path.clone())?;
        extract(File::open(path)?, session.library.root())?;
        session.validate()?;
        Ok(Self {
            saved: 1,
            ..session
        })
    }
    pub fn audio(&self, source: &AudioSource) -> Result<Vec<u8>> {
        match source {
            AudioSource::Take(id) => Ok(self.library.audio(id)?),
            AudioSource::Preview(id) => Ok(self.previews.audio(id)?),
            AudioSource::CharaPreview(id) => {
                let path = self
                    .library
                    .chara_preview_path(id)?
                    .ok_or_else(|| anyhow::anyhow!("サンプル音声がありません"))?;
                Ok(std::fs::read(path)?)
            }
        }
    }
    pub(super) fn validate(&self) -> Result<()> {
        let editor = self.library.editor()?;
        let charas = self.library.charas()?;
        let voices = self.library.voices()?;
        let takes = self.library.takes()?;
        for line in &editor.lines {
            if !line.chara_id.is_empty() && !charas.iter().any(|c| c.id == line.chara_id) {
                bail!("キャラクタが見つかりません");
            }
            if let Some(id) = &line.take_id
                && !takes
                    .iter()
                    .any(|take| &take.id == id && line.takes.contains(&take.id))
            {
                bail!("生成音声が見つかりません");
            }
        }
        for voice in &voices {
            crate::audio::validate_wav(&self.library.audio(voice.id())?)?;
        }
        for chara in charas {
            for wav in self.library.chara_reference_audio(&chara)? {
                crate::audio::validate_wav(&wav)?;
            }
        }
        Ok(())
    }
    pub fn changed(&mut self) {
        self.revision += 1;
        self.error = None;
    }
    pub(super) fn status(&self) -> ProjectStatus {
        ProjectStatus {
            id: Some(self.id.clone()),
            name: Some(
                self.path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            ),
            path: Some(self.path.to_string_lossy().into_owned()),
            state: if self.error.is_some() {
                SaveState::Error
            } else if self.revision != self.saved {
                SaveState::Saving
            } else {
                SaveState::Saved
            },
            error: self.error.clone(),
        }
    }
}

pub type SharedProject = Arc<Mutex<Option<Session>>>;
pub fn status(project: &SharedProject) -> Result<ProjectStatus> {
    Ok(project
        .lock()
        .map_err(|_| anyhow::anyhow!("project lock poisoned"))?
        .as_ref()
        .map(Session::status)
        .unwrap_or(ProjectStatus {
            id: None,
            name: None,
            path: None,
            state: SaveState::Closed,
            error: None,
        }))
}
