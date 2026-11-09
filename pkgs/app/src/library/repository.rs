use std::path::{Path as FsPath, PathBuf};

use anyhow::Result;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::domain::{AppError, Character, CharacterInput, Take, new_id, valid_id};
use crate::files::atomic_bytes;

pub struct Library {
    root: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct ProjectDocument {
    version: u32,
    #[serde(flatten)]
    editor: crate::domain::EditorState,
}

fn check_id(id: &str) -> Result<(), AppError> {
    if valid_id(id) {
        Ok(())
    } else {
        Err(AppError::not_found("not found"))
    }
}
fn read_yaml<T: serde::de::DeserializeOwned>(path: &FsPath) -> Result<T, AppError> {
    let bytes = std::fs::read(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            AppError::not_found("not found")
        } else {
            AppError::internal(e)
        }
    })?;
    serde_yaml::from_slice(&bytes).map_err(AppError::internal)
}

impl Library {
    pub fn open(root: &FsPath) -> Result<Self> {
        std::fs::create_dir_all(root.join("characters"))?;
        std::fs::create_dir_all(root.join("voices"))?;
        std::fs::create_dir_all(root.join(".imports"))?;
        Ok(Self {
            root: root.to_owned(),
        })
    }
    fn atomic_yaml(&self, path: &FsPath, value: &impl Serialize) -> Result<(), AppError> {
        atomic_bytes(
            path,
            serde_yaml::to_string(value)
                .map_err(AppError::internal)?
                .as_bytes(),
        )
    }
    pub fn root(&self) -> &FsPath {
        &self.root
    }
    pub fn editor(&self) -> Result<crate::domain::EditorState, AppError> {
        let path = self.root.join("project.yaml");
        if !path.exists() {
            return Ok(crate::domain::EditorState::default());
        }
        let project: ProjectDocument = read_yaml(&path)?;
        if project.version != 1 {
            return Err(AppError::invalid("未対応のプロジェクト形式です"));
        }
        Ok(project.editor)
    }
    pub fn save_editor(&self, state: &crate::domain::EditorState) -> Result<(), AppError> {
        let takes = self.takes()?;
        let mut editor = state.clone();
        let saved = self.editor()?;
        // Keep explicit take selection authoritative while older UI saves are queued.
        for row in &mut editor.rows {
            row.take_id = saved
                .rows
                .iter()
                .find(|saved| saved.id == row.id)
                .and_then(|saved| saved.take_id.clone())
                .filter(|id| {
                    takes
                        .iter()
                        .any(|take| &take.id == id && take.row_id == row.id)
                });
        }
        self.write_editor(&editor)?;
        for take in takes {
            if !editor.rows.iter().any(|row| row.id == take.row_id) {
                self.delete_take(&take.id)?;
            }
        }
        Ok(())
    }
    fn write_editor(&self, editor: &crate::domain::EditorState) -> Result<(), AppError> {
        self.atomic_yaml(
            &self.root.join("project.yaml"),
            &ProjectDocument {
                version: 1,
                editor: editor.clone(),
            },
        )
    }
    pub fn voices(&self, characters: &[Character]) -> Result<Vec<String>, AppError> {
        let mut names: Vec<_> = characters.iter().filter_map(|c| c.voice.clone()).collect();
        for entry in std::fs::read_dir(self.root.join(".imports")).map_err(AppError::internal)? {
            let path = entry.map_err(AppError::internal)?.path();
            if path.extension().is_some_and(|e| e == "wav") {
                names.push(path.file_stem().unwrap().to_string_lossy().into_owned());
            }
        }
        names.sort();
        names.dedup();
        Ok(names)
    }
    fn voice_path(&self, name: &str) -> Result<PathBuf, AppError> {
        if let Some(id) = name.strip_suffix("/reference.wav") {
            check_id(id)?;
            return Ok(self.root.join("characters").join(id).join("reference.wav"));
        }
        if name.is_empty()
            || name.chars().count() > 64
            || name.starts_with('.')
            || name.chars().any(|c| {
                c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
            })
        {
            return Err(AppError::invalid("invalid reference voice name"));
        }
        Ok(self.root.join(".imports").join(format!("{name}.wav")))
    }
    pub fn voice(&self, name: &str) -> Result<Vec<u8>, AppError> {
        std::fs::read(self.voice_path(name)?).map_err(AppError::internal)
    }
    pub fn save_voice(&self, name: &str, wav: &[u8]) -> Result<(), AppError> {
        use std::io::Write;
        crate::audio::validate_wav(wav).map_err(AppError::invalid)?;
        let path = self.voice_path(name)?;
        let mut temp =
            tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(AppError::internal)?;
        temp.write_all(wav).map_err(AppError::internal)?;
        temp.as_file().sync_all().map_err(AppError::internal)?;
        temp.persist_noclobber(path).map_err(AppError::internal)?;
        Ok(())
    }
    pub fn character(&self, id: &str) -> Result<Character, AppError> {
        check_id(id)?;
        let dir = self.root.join("characters").join(id);
        let mut character: Character = read_yaml(&dir.join("character.yaml"))?;
        if character.id != id {
            return Err(AppError::invalid("キャラクタ ID が一致しません"));
        }
        if let Some(voice) = &character.voice {
            if voice != "reference.wav" {
                return Err(AppError::invalid("invalid reference path"));
            }
            character.voice = Some(format!("{id}/reference.wav"));
        }
        if let Some(icon) = &character.icon {
            if icon != "icon.png" {
                return Err(AppError::invalid("invalid icon path"));
            }
            character.icon = Some(format!(
                "data:image/png;base64,{}",
                STANDARD.encode(std::fs::read(dir.join(icon)).map_err(AppError::internal)?)
            ));
        }
        Ok(character)
    }
    pub fn characters(&self) -> Result<Vec<Character>, AppError> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(self.root.join("characters")).map_err(AppError::internal)? {
            let entry = entry.map_err(AppError::internal)?;
            if let Some(id) = entry.file_name().to_str().filter(|id| valid_id(id)) {
                out.push(self.character(id)?);
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    pub fn create_character(&self, input: CharacterInput) -> Result<Character, AppError> {
        self.save_character(new_id(), input)
    }

    pub fn update_character(
        &self,
        id: String,
        mut input: CharacterInput,
    ) -> Result<Character, AppError> {
        let existing = self.character(&id)?;
        if input.color.is_none() {
            input.color = Some(existing.color);
        }
        if input.sample_text.is_none() {
            input.sample_text = Some(existing.sample_text);
        }
        if input.icon.is_none() {
            input.icon = existing.icon;
        }
        self.save_character(id, input)
    }

    pub fn delete_character(&self, id: &str) -> Result<(), AppError> {
        self.character(id)?;
        std::fs::remove_dir_all(self.root.join("characters").join(id))
            .map_err(AppError::internal)?;
        let mut editor = self.editor()?;
        for row in &mut editor.rows {
            if row.character_id == id {
                row.character_id.clear();
            }
        }
        if editor.active_character == id {
            editor.active_character.clear();
        }
        self.write_editor(&editor)
    }

    fn save_character(&self, id: String, input: CharacterInput) -> Result<Character, AppError> {
        check_id(&id)?;
        let character = input.normalize(id)?;
        if let Some(voice) = &character.voice {
            self.voice(voice)?;
        }
        let icon = &character.icon;
        if let Some(value) = icon {
            let data = value
                .strip_prefix("data:image/png;base64,")
                .filter(|data| data.len() <= 150_000)
                .and_then(|data| STANDARD.decode(data).ok());
            if !data.is_some_and(|data| data.starts_with(b"\x89PNG\r\n\x1a\n")) {
                return Err(AppError::invalid("invalid character icon"));
            }
        }
        let dir = self.root.join("characters").join(&character.id);
        std::fs::create_dir_all(&dir).map_err(AppError::internal)?;
        let mut stored = character.clone();
        if let Some(voice) = &character.voice {
            atomic_bytes(&dir.join("reference.wav"), &self.voice(voice)?)?;
            stored.voice = Some("reference.wav".into());
        }
        if let Some(icon) = &character.icon {
            atomic_bytes(
                &dir.join("icon.png"),
                &STANDARD
                    .decode(icon.strip_prefix("data:image/png;base64,").unwrap())
                    .map_err(AppError::internal)?,
            )?;
            stored.icon = Some("icon.png".into());
        }
        self.atomic_yaml(&dir.join("character.yaml"), &stored)?;
        for (keep, name) in [
            (stored.voice.is_some(), "reference.wav"),
            (stored.icon.is_some(), "icon.png"),
        ] {
            if !keep && dir.join(name).exists() {
                std::fs::remove_file(dir.join(name)).map_err(AppError::internal)?;
            }
        }
        self.character(&character.id)
    }
    pub fn save_character_preview(
        &self,
        id: &str,
        source: &Library,
        preview: &str,
    ) -> Result<(), AppError> {
        self.character(id)?;
        let take: Take = read_yaml(&source.take_dir(preview)?.join("meta.yaml"))?;
        let dir = self.root.join("characters").join(id).join("previews");
        std::fs::create_dir_all(&dir).map_err(AppError::internal)?;
        atomic_bytes(&dir.join("audio.wav"), &source.audio(preview)?)?;
        self.atomic_yaml(&dir.join("meta.yaml"), &take)
    }
    pub fn character_preview_path(&self, id: &str) -> Result<Option<PathBuf>, AppError> {
        check_id(id)?;
        let path = self
            .root
            .join("characters")
            .join(id)
            .join("previews/audio.wav");
        Ok(path.is_file().then_some(path))
    }
    pub fn takes(&self) -> Result<Vec<Take>, AppError> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(self.root.join("voices")).map_err(AppError::internal)? {
            let entry = entry.map_err(AppError::internal)?;
            if entry.file_name().to_str().is_some_and(valid_id) {
                let take: Take = read_yaml(&entry.path().join("meta.yaml"))?;
                if entry.file_name().to_str() != Some(&take.id) {
                    return Err(AppError::invalid("生成音声 ID が一致しません"));
                }
                out.push(take);
            }
        }
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
        Ok(out)
    }
    pub fn save_take(&self, take: &Take, wav: &[u8]) -> Result<(), AppError> {
        check_id(&take.id)?;
        let dir = self.root.join("voices");
        let temp = dir.join(format!(".{}", take.id));
        let write = || -> Result<()> {
            std::fs::create_dir(&temp)?;
            std::fs::write(temp.join("audio.wav"), wav)?;
            std::fs::write(
                temp.join("meta.yaml"),
                serde_yaml::to_string(take)?.as_bytes(),
            )?;
            std::fs::rename(&temp, dir.join(&take.id))?;
            Ok(())
        };
        if let Err(e) = write() {
            let _ = std::fs::remove_dir_all(temp);
            return Err(AppError::internal(e));
        }
        let mut editor = self.editor()?;
        for row in &mut editor.rows {
            if row.id == take.row_id {
                take.restore_row(row, &editor.preferences);
            }
        }
        self.write_editor(&editor)?;
        Ok(())
    }

    pub fn select_take(&self, row_id: &str, take_id: Option<&str>) -> Result<(), AppError> {
        let take = if let Some(id) = take_id {
            let take: Take = read_yaml(&self.take_dir(id)?.join("meta.yaml"))?;
            if take.row_id != row_id {
                return Err(AppError::invalid("このセリフのテイクではありません"));
            }
            Some(take)
        } else {
            None
        };
        let mut editor = self.editor()?;
        let row = editor
            .rows
            .iter_mut()
            .find(|row| row.id == row_id)
            .ok_or_else(|| AppError::invalid("セリフが見つかりません"))?;
        if let Some(take) = take {
            take.restore_row(row, &editor.preferences);
        } else {
            row.take_id = None;
        }
        self.write_editor(&editor)
    }

    fn take_dir(&self, id: &str) -> Result<PathBuf, AppError> {
        check_id(id)?;
        let dir = self.root.join("voices").join(id);
        let _: Take = read_yaml(&dir.join("meta.yaml"))?;
        Ok(dir)
    }

    pub fn audio(&self, id: &str) -> Result<Vec<u8>, AppError> {
        std::fs::read(self.take_dir(id)?.join("audio.wav")).map_err(AppError::internal)
    }

    pub fn delete_take(&self, id: &str) -> Result<(), AppError> {
        std::fs::remove_dir_all(self.take_dir(id)?).map_err(AppError::internal)?;
        let mut editor = self.editor()?;
        for row in &mut editor.rows {
            if row.take_id.as_deref() == Some(id) {
                row.take_id = None;
            }
        }
        self.write_editor(&editor)
    }
}

#[cfg(test)]
mod tests;
