use std::path::{Path as FsPath, PathBuf};

use anyhow::Result;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::domain::{AppError, Character, CharacterInput, Take, Voice, new_id, valid_id};
use crate::files::atomic_bytes;

pub struct Library {
    root: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct ProjectDocument {
    version: u32,
    scripts: Vec<String>,
    preferences: crate::domain::GenerationSettings,
    active_character: String,
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
        std::fs::create_dir_all(root.join("scripts"))?;
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
        if project.version != 2 {
            return Err(AppError::invalid("未対応のプロジェクト形式です"));
        }
        let mut rows = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for id in project.scripts {
            if !seen.insert(id.clone()) {
                return Err(AppError::invalid("台本の行 ID が重複しています"));
            }
            let row: crate::domain::ScriptRow = read_yaml(&self.script_path(&id)?)?;
            if row.id != id {
                return Err(AppError::invalid("台本の行 ID が一致しません"));
            }
            rows.push(row);
        }
        Ok(crate::domain::EditorState {
            rows,
            preferences: project.preferences,
            active_character: project.active_character,
        })
    }
    fn script_path(&self, id: &str) -> Result<PathBuf, AppError> {
        if !valid_script_id(id) {
            return Err(AppError::invalid("台本の行 ID が不正です"));
        }
        Ok(self.root.join("scripts").join(format!("{id}.yaml")))
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
        self.write_editor(&editor)
    }
    fn write_editor(&self, editor: &crate::domain::EditorState) -> Result<(), AppError> {
        let mut seen = std::collections::HashSet::new();
        for row in &editor.rows {
            self.script_path(&row.id)?;
            if !seen.insert(&row.id) {
                return Err(AppError::invalid("台本の行 ID が重複しています"));
            }
        }
        let staging = tempfile::Builder::new()
            .prefix(".scripts-")
            .tempdir_in(&self.root)
            .map_err(AppError::internal)?;
        let next = staging.path().join("next");
        let previous = staging.path().join("previous");
        std::fs::create_dir(&next).map_err(AppError::internal)?;
        for row in &editor.rows {
            self.atomic_yaml(&next.join(format!("{}.yaml", row.id)), row)?;
        }
        let scripts = self.root.join("scripts");
        std::fs::rename(&scripts, &previous).map_err(AppError::internal)?;
        let result = (|| {
            std::fs::rename(&next, &scripts).map_err(AppError::internal)?;
            self.atomic_yaml(
                &self.root.join("project.yaml"),
                &ProjectDocument {
                    version: 2,
                    scripts: editor.rows.iter().map(|r| r.id.clone()).collect(),
                    preferences: editor.preferences.clone(),
                    active_character: editor.active_character.clone(),
                },
            )
        })();
        if result.is_err() {
            if scripts.exists() {
                std::fs::remove_dir_all(&scripts).map_err(AppError::internal)?;
            }
            std::fs::rename(&previous, &scripts).map_err(AppError::internal)?;
        }
        result
    }

    pub fn voices(&self) -> Result<Vec<Voice>, AppError> {
        let mut voices = Vec::new();
        for entry in std::fs::read_dir(self.root.join("voices")).map_err(AppError::internal)? {
            let entry = entry.map_err(AppError::internal)?;
            if let Some(id) = entry
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .filter(|id| valid_id(id))
                && entry.path().extension().is_some_and(|ext| ext == "wav")
            {
                voices.push(self.voice_record(id)?);
            }
        }
        voices.sort_by_key(|v| v.id().to_owned());
        Ok(voices)
    }
    pub fn voice_record(&self, id: &str) -> Result<Voice, AppError> {
        check_id(id)?;
        let mut file = std::fs::File::open(self.voice_path(id)?).map_err(AppError::internal)?;
        let voice = crate::audio::read_voice(&mut file)
            .map_err(AppError::invalid)?
            .ok_or_else(|| AppError::invalid("WAV に音声情報がありません"))?;
        if voice.id() != id {
            return Err(AppError::invalid("音声 ID が一致しません"));
        }
        Ok(voice)
    }
    pub fn reference_audio(&self, ids: &[String]) -> Result<Vec<Vec<u8>>, AppError> {
        ids.iter().map(|id| self.audio(id)).collect()
    }
    pub fn save_voice(&self, name: &str, wav: &[u8]) -> Result<String, AppError> {
        let duration = crate::audio::validate_wav(wav).map_err(AppError::invalid)?;
        let id = new_id();
        let mut voice = crate::audio::read_voice(&mut std::io::Cursor::new(wav))
            .map_err(AppError::invalid)?
            .unwrap_or_else(|| Voice::Imported {
                id: id.clone(),
                name: name.into(),
            });
        match &mut voice {
            Voice::Imported { id: stored, .. } => *stored = id.clone(),
            Voice::Generated(take) => {
                take.id = id.clone();
                take.audio_seconds = duration;
            }
        }
        self.store_voice(&voice, wav)?;
        Ok(id)
    }
    fn store_voice(&self, voice: &Voice, wav: &[u8]) -> Result<(), AppError> {
        let path = self.voice_path(voice.id())?;
        if path.exists() {
            return Err(AppError::invalid("音声 ID が重複しています"));
        }
        let wav = crate::audio::with_voice(wav, voice).map_err(AppError::invalid)?;
        atomic_bytes(&path, &wav)
    }
    fn voice_path(&self, id: &str) -> Result<PathBuf, AppError> {
        check_id(id)?;
        Ok(self.root.join("voices").join(format!("{id}.wav")))
    }
    pub fn character(&self, id: &str) -> Result<Character, AppError> {
        check_id(id)?;
        let dir = self.root.join("characters").join(id);
        let mut character: Character = read_yaml(&dir.join("character.yaml"))?;
        if character.id != id {
            return Err(AppError::invalid("キャラクタ ID が一致しません"));
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
        for voice in &character.voice_ids {
            self.voice_record(voice)?;
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
        if stored.icon.is_none() && dir.join("icon.png").exists() {
            std::fs::remove_file(dir.join("icon.png")).map_err(AppError::internal)?;
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
        atomic_bytes(
            &self.root.join("characters").join(id).join("preview.wav"),
            &source.audio(preview)?,
        )
    }
    pub fn character_preview_path(&self, id: &str) -> Result<Option<PathBuf>, AppError> {
        check_id(id)?;
        let path = self.root.join("characters").join(id).join("preview.wav");
        Ok(path.is_file().then_some(path))
    }
    pub fn takes(&self) -> Result<Vec<Take>, AppError> {
        let mut takes: Vec<_> = self
            .voices()?
            .into_iter()
            .filter_map(|v| match v {
                Voice::Generated(take) => Some(*take),
                Voice::Imported { .. } => None,
            })
            .collect();
        takes.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
        Ok(takes)
    }
    pub fn save_take(&self, take: &Take, wav: &[u8]) -> Result<(), AppError> {
        self.store_voice(&Voice::Generated(Box::new(take.clone())), wav)?;
        let mut editor = self.editor()?;
        for row in &mut editor.rows {
            if row.id == take.row_id {
                take.restore_row(row);
            }
        }
        self.write_editor(&editor)?;
        Ok(())
    }

    pub fn select_take(&self, row_id: &str, take_id: Option<&str>) -> Result<(), AppError> {
        let take = if let Some(id) = take_id {
            let Voice::Generated(take) = self.voice_record(id)? else {
                return Err(AppError::invalid("生成音声ではありません"));
            };
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
            take.restore_row(row);
        } else {
            row.take_id = None;
        }
        self.write_editor(&editor)
    }

    pub fn audio(&self, id: &str) -> Result<Vec<u8>, AppError> {
        std::fs::read(self.voice_path(id)?).map_err(AppError::internal)
    }

    pub fn delete_take(&self, id: &str) -> Result<(), AppError> {
        if self
            .characters()?
            .iter()
            .any(|c| c.voice_ids.iter().any(|v| v == id))
        {
            return Err(AppError::invalid(
                "キャラクターが参照している音声は削除できません。先に参照を解除してください",
            ));
        }
        std::fs::remove_file(self.voice_path(id)?).map_err(AppError::internal)?;
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

// Script IDs are used as filenames; exclude path components.
pub(crate) fn valid_script_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

mod transfer;
