use std::path::{Path as FsPath, PathBuf};

use anyhow::Result;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::domain::{AppError, Chara, CharaInput, Take, Voice, new_id, valid_id};
use crate::files::atomic_bytes;

pub struct Library {
    root: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct ProjectDocument {
    version: u32,
    lines: indexmap::IndexMap<String, StoredLine>,
}

#[derive(Serialize, Deserialize)]
struct StoredLine {
    takes: Vec<String>,
    text: String,
    chara_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    voice_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    settings: Option<crate::domain::GenerationSettings>,
}

impl From<crate::domain::Line> for StoredLine {
    fn from(line: crate::domain::Line) -> Self {
        Self {
            takes: line.takes,
            text: line.text,
            chara_id: line.chara_id,
            voice_id: line.take_id,
            prompt: line.prompt,
            settings: line.settings,
        }
    }
}

impl StoredLine {
    fn into_line(self, id: String) -> crate::domain::Line {
        crate::domain::Line {
            id,
            takes: self.takes,
            text: self.text,
            chara_id: self.chara_id,
            take_id: self.voice_id,
            prompt: self.prompt,
            settings: self.settings,
        }
    }
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
        std::fs::create_dir_all(root.join("charas"))?;
        std::fs::create_dir_all(root.join("voices"))?;
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
        if project.version != 0 {
            return Err(AppError::invalid("未対応のプロジェクト形式です"));
        }
        let lines = project
            .lines
            .into_iter()
            .map(|(id, line)| line.into_line(id))
            .collect::<Vec<_>>();
        validate_lines(&lines)?;
        Ok(crate::domain::EditorState {
            lines,
            active_chara: String::new(),
        })
    }
    pub fn save_editor(&self, state: &crate::domain::EditorState) -> Result<(), AppError> {
        let mut editor = state.clone();
        let saved = self.editor()?;
        // Generation and explicit take selection own the history; stale UI saves only edit text/settings.
        for line in &mut editor.lines {
            if let Some(saved) = saved.lines.iter().find(|saved| saved.id == line.id) {
                line.takes = saved.takes.clone();
                line.take_id = saved.take_id.clone();
            } else {
                line.takes.clear();
                line.take_id = None;
            }
        }
        self.write_editor(&editor)
    }
    fn write_editor(&self, editor: &crate::domain::EditorState) -> Result<(), AppError> {
        validate_lines(&editor.lines)?;
        self.atomic_yaml(
            &self.root.join("project.yaml"),
            &ProjectDocument {
                version: 0,
                lines: editor
                    .lines
                    .iter()
                    .cloned()
                    .map(|line| (line.id.clone(), line.into()))
                    .collect(),
            },
        )
    }

    fn stored_voices(&self) -> Result<Vec<Voice>, AppError> {
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
                voices.push(Self::read_voice_record(&entry.path(), id)?);
            }
        }
        Ok(voices)
    }
    /// Available recordings include each chara's independent reference copies.
    pub fn voices(&self) -> Result<Vec<Voice>, AppError> {
        let mut voices = std::collections::BTreeMap::new();
        for voice in self.stored_voices()? {
            voices.insert(voice.id().to_owned(), voice);
        }
        for chara in self.charas()? {
            for id in &chara.voice_ids {
                let voice = Self::read_voice_record(&self.chara_voice_path(&chara.id, id)?, id)?;
                voices.entry(id.clone()).or_insert(voice);
            }
        }
        Ok(voices.into_values().collect())
    }
    fn read_voice_record(path: &FsPath, id: &str) -> Result<Voice, AppError> {
        let mut file = std::fs::File::open(path).map_err(AppError::internal)?;
        let voice = crate::audio::read_voice(&mut file)
            .map_err(AppError::invalid)?
            .ok_or_else(|| AppError::invalid("WAV に音声情報がありません"))?;
        if voice.id() != id {
            return Err(AppError::invalid("音声 ID が一致しません"));
        }
        Ok(voice)
    }
    pub fn voice_record(&self, id: &str) -> Result<Voice, AppError> {
        Self::read_voice_record(&self.available_voice_path(id)?, id)
    }
    fn chara_voice_path(&self, chara: &str, id: &str) -> Result<PathBuf, AppError> {
        check_id(chara)?;
        check_id(id)?;
        Ok(self
            .root
            .join("charas")
            .join(chara)
            .join("voices")
            .join(format!("{id}.wav")))
    }
    fn available_voice_path(&self, id: &str) -> Result<PathBuf, AppError> {
        let path = self.voice_path(id)?;
        if path.is_file() {
            return Ok(path);
        }
        for chara in self.charas()? {
            if chara.voice_ids.iter().any(|voice| voice == id) {
                return self.chara_voice_path(&chara.id, id);
            }
        }
        Err(AppError::not_found("音声が見つかりません"))
    }
    pub fn chara_reference_audio(&self, chara: &Chara) -> Result<Vec<Vec<u8>>, AppError> {
        chara
            .voice_ids
            .iter()
            .map(|id| {
                let path = self.chara_voice_path(&chara.id, id)?;
                Self::read_voice_record(&path, id)?;
                std::fs::read(path).map_err(AppError::internal)
            })
            .collect()
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
    pub fn chara(&self, id: &str) -> Result<Chara, AppError> {
        check_id(id)?;
        let dir = self.root.join("charas").join(id);
        let mut chara: Chara = read_yaml(&dir.join("chara.yaml"))?;
        if chara.id != id {
            return Err(AppError::invalid("キャラクタ ID が一致しません"));
        }
        if let Some(icon) = &chara.icon {
            if icon != "icon.png" {
                return Err(AppError::invalid("invalid icon path"));
            }
            chara.icon = Some(format!(
                "data:image/png;base64,{}",
                STANDARD.encode(std::fs::read(dir.join(icon)).map_err(AppError::internal)?)
            ));
        }
        Ok(chara)
    }
    pub fn charas(&self) -> Result<Vec<Chara>, AppError> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(self.root.join("charas")).map_err(AppError::internal)? {
            let entry = entry.map_err(AppError::internal)?;
            if let Some(id) = entry.file_name().to_str().filter(|id| valid_id(id)) {
                out.push(self.chara(id)?);
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    pub fn create_chara(&self, input: CharaInput) -> Result<Chara, AppError> {
        self.save_chara(new_id(), input)
    }

    pub fn update_chara(&self, id: String, mut input: CharaInput) -> Result<Chara, AppError> {
        let existing = self.chara(&id)?;
        if input.color.is_none() {
            input.color = Some(existing.color);
        }
        if input.sample_text.is_none() {
            input.sample_text = Some(existing.sample_text);
        }
        if input.icon.is_none() {
            input.icon = existing.icon;
        }
        self.save_chara(id, input)
    }

    pub fn delete_chara(&self, id: &str) -> Result<(), AppError> {
        self.chara(id)?;
        std::fs::remove_dir_all(self.root.join("charas").join(id)).map_err(AppError::internal)?;
        let mut editor = self.editor()?;
        for line in &mut editor.lines {
            if line.chara_id == id {
                line.chara_id.clear();
            }
        }
        self.write_editor(&editor)
    }

    fn save_chara(&self, id: String, input: CharaInput) -> Result<Chara, AppError> {
        self.save_chara_from(id, input, self)
    }

    fn save_chara_from(
        &self,
        id: String,
        input: CharaInput,
        source: &Library,
    ) -> Result<Chara, AppError> {
        check_id(&id)?;
        let chara = input.normalize(id)?;
        let icon = &chara.icon;
        if let Some(value) = icon {
            let data = value
                .strip_prefix("data:image/png;base64,")
                .filter(|data| data.len() <= 150_000)
                .and_then(|data| STANDARD.decode(data).ok());
            if !data.is_some_and(|data| data.starts_with(b"\x89PNG\r\n\x1a\n")) {
                return Err(AppError::invalid("invalid chara icon"));
            }
        }
        let dir = self.root.join("charas").join(&chara.id);
        let staging = tempfile::Builder::new()
            .prefix(".chara-")
            .tempdir_in(&self.root)
            .map_err(AppError::internal)?;
        let next = staging.path().join("next");
        let previous = staging.path().join("previous");
        std::fs::create_dir_all(next.join("voices")).map_err(AppError::internal)?;
        for voice in &chara.voice_ids {
            source.voice_record(voice)?;
            atomic_bytes(
                &next.join("voices").join(format!("{voice}.wav")),
                &source.audio(voice)?,
            )?;
        }
        if dir.join("preview.wav").is_file() {
            std::fs::copy(dir.join("preview.wav"), next.join("preview.wav"))
                .map_err(AppError::internal)?;
        }
        let mut stored = chara.clone();
        if let Some(icon) = &chara.icon {
            atomic_bytes(
                &next.join("icon.png"),
                &STANDARD
                    .decode(icon.strip_prefix("data:image/png;base64,").unwrap())
                    .map_err(AppError::internal)?,
            )?;
            stored.icon = Some("icon.png".into());
        }
        self.atomic_yaml(&next.join("chara.yaml"), &stored)?;
        let replacing = dir.exists();
        if replacing {
            std::fs::rename(&dir, &previous).map_err(AppError::internal)?;
        }
        if let Err(error) = std::fs::rename(&next, &dir) {
            if replacing {
                std::fs::rename(&previous, &dir).map_err(AppError::internal)?;
            }
            return Err(AppError::internal(error));
        }
        self.chara(&chara.id)
    }
    pub fn save_chara_preview(
        &self,
        id: &str,
        source: &Library,
        preview: &str,
    ) -> Result<(), AppError> {
        self.chara(id)?;
        atomic_bytes(
            &self.root.join("charas").join(id).join("preview.wav"),
            &source.audio(preview)?,
        )
    }
    pub fn chara_preview_path(&self, id: &str) -> Result<Option<PathBuf>, AppError> {
        check_id(id)?;
        let path = self.root.join("charas").join(id).join("preview.wav");
        Ok(path.is_file().then_some(path))
    }
    pub fn takes(&self) -> Result<Vec<Take>, AppError> {
        self.editor()?
            .lines
            .into_iter()
            .flat_map(|line| line.takes)
            .map(
                |id| match Self::read_voice_record(&self.voice_path(&id)?, &id)? {
                    Voice::Generated(take) => Ok(*take),
                    Voice::Imported { .. } => {
                        Err(AppError::invalid("テイクの参照先が生成音声ではありません"))
                    }
                },
            )
            .collect()
    }
    pub fn save_take(&self, take: &Take, wav: &[u8]) -> Result<(), AppError> {
        let mut editor = self.editor()?;
        self.store_voice(&Voice::Generated(Box::new(take.clone())), wav)?;
        for line in &mut editor.lines {
            if line.id == take.line_id {
                line.takes.insert(0, take.id.clone());
                take.restore_line(line);
            }
        }
        if let Err(error) = self.write_editor(&editor) {
            std::fs::remove_file(self.voice_path(&take.id)?).map_err(AppError::internal)?;
            return Err(error);
        }
        Ok(())
    }

    pub fn select_take(&self, line_id: &str, take_id: Option<&str>) -> Result<(), AppError> {
        let take = if let Some(id) = take_id {
            let Voice::Generated(take) = Self::read_voice_record(&self.voice_path(id)?, id)? else {
                return Err(AppError::invalid("生成音声ではありません"));
            };
            Some(take)
        } else {
            None
        };
        let mut editor = self.editor()?;
        let line = editor
            .lines
            .iter_mut()
            .find(|line| line.id == line_id)
            .ok_or_else(|| AppError::invalid("セリフが見つかりません"))?;
        if let Some(take) = take {
            if !line.takes.contains(&take.id) {
                return Err(AppError::invalid("このセリフのテイクではありません"));
            }
            take.restore_line(line);
        } else {
            line.take_id = None;
        }
        self.write_editor(&editor)
    }

    pub fn audio(&self, id: &str) -> Result<Vec<u8>, AppError> {
        std::fs::read(self.available_voice_path(id)?).map_err(AppError::internal)
    }

    pub fn delete_take(&self, id: &str) -> Result<(), AppError> {
        let mut editor = self.editor()?;
        for line in &mut editor.lines {
            line.takes.retain(|take| take != id);
            if line.take_id.as_deref() == Some(id) {
                line.take_id = None;
            }
        }
        self.write_editor(&editor)?;
        std::fs::remove_file(self.voice_path(id)?).map_err(AppError::internal)
    }
}

fn validate_lines(lines: &[crate::domain::Line]) -> Result<(), AppError> {
    let mut seen = std::collections::HashSet::new();
    let mut takes = std::collections::HashSet::new();
    for line in lines {
        for id in &line.takes {
            check_id(id)?;
            if !takes.insert(id) {
                return Err(AppError::invalid("テイク ID が重複しています"));
            }
        }
        if line
            .take_id
            .as_ref()
            .is_some_and(|id| !line.takes.contains(id))
        {
            return Err(AppError::invalid(
                "選択中のテイクがセリフの一覧にありません",
            ));
        }
        if line.id.is_empty() || line.id.len() > 64 {
            return Err(AppError::invalid("セリフ ID が不正です"));
        }
        if !seen.insert(&line.id) {
            return Err(AppError::invalid("セリフ ID が重複しています"));
        }
    }
    Ok(())
}

mod transfer;
