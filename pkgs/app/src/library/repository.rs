use std::path::{Path as FsPath, PathBuf};

use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::domain::{Chara, CharaInput, Take, Voice, new_id, valid_id};
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

fn check_id(id: &str) -> Result<()> {
    if valid_id(id) {
        Ok(())
    } else {
        Err(anyhow!("not found"))
    }
}
fn read_yaml<T: serde::de::DeserializeOwned>(path: &FsPath) -> Result<T> {
    let bytes = std::fs::read(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            anyhow!("not found")
        } else {
            e.into()
        }
    })?;
    Ok(serde_yaml::from_slice(&bytes)?)
}

impl Library {
    pub fn open(root: &FsPath) -> Result<Self> {
        std::fs::create_dir_all(root.join("charas"))?;
        std::fs::create_dir_all(root.join("voices"))?;
        Ok(Self {
            root: root.to_owned(),
        })
    }
    fn atomic_yaml(&self, path: &FsPath, value: &impl Serialize) -> Result<()> {
        atomic_bytes(path, serde_yaml::to_string(value)?.as_bytes())
    }
    pub fn root(&self) -> &FsPath {
        &self.root
    }
    pub fn editor(&self) -> Result<crate::domain::EditorState> {
        let path = self.root.join("project.yaml");
        if !path.exists() {
            return Ok(crate::domain::EditorState::default());
        }
        let project: ProjectDocument = read_yaml(&path)?;
        if project.version != 0 {
            return Err(anyhow!("未対応のプロジェクト形式です"));
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
    pub fn save_editor(&self, state: &crate::domain::EditorState) -> Result<()> {
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
    fn write_editor(&self, editor: &crate::domain::EditorState) -> Result<()> {
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

    fn stored_voices(&self) -> Result<Vec<Voice>> {
        let mut voices = Vec::new();
        for entry in std::fs::read_dir(self.root.join("voices"))? {
            let entry = entry?;
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
    pub fn voices(&self) -> Result<Vec<Voice>> {
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
    fn read_voice_record(path: &FsPath, id: &str) -> Result<Voice> {
        let mut file = std::fs::File::open(path)?;
        let voice = crate::audio::read_voice(&mut file)?
            .ok_or_else(|| anyhow!("WAV に音声情報がありません"))?;
        if voice.id() != id {
            return Err(anyhow!("音声 ID が一致しません"));
        }
        Ok(voice)
    }
    pub fn voice_record(&self, id: &str) -> Result<Voice> {
        Self::read_voice_record(&self.available_voice_path(id)?, id)
    }
    fn chara_voice_path(&self, chara: &str, id: &str) -> Result<PathBuf> {
        check_id(chara)?;
        check_id(id)?;
        Ok(self
            .root
            .join("charas")
            .join(chara)
            .join("voices")
            .join(format!("{id}.wav")))
    }
    fn available_voice_path(&self, id: &str) -> Result<PathBuf> {
        let path = self.voice_path(id)?;
        if path.is_file() {
            return Ok(path);
        }
        for chara in self.charas()? {
            if chara.voice_ids.iter().any(|voice| voice == id) {
                return self.chara_voice_path(&chara.id, id);
            }
        }
        Err(anyhow!("音声が見つかりません"))
    }
    pub fn chara_reference_audio(&self, chara: &Chara) -> Result<Vec<Vec<u8>>> {
        chara
            .voice_ids
            .iter()
            .map(|id| {
                let path = self.chara_voice_path(&chara.id, id)?;
                Self::read_voice_record(&path, id)?;
                Ok(std::fs::read(path)?)
            })
            .collect()
    }
    pub fn save_voice(&self, name: &str, wav: &[u8]) -> Result<String> {
        let duration = crate::audio::validate_wav(wav).map_err(|e| anyhow!("{e:#}"))?;
        let id = new_id();
        let mut voice =
            crate::audio::read_voice(&mut std::io::Cursor::new(wav))?.unwrap_or_else(|| {
                Voice::Imported {
                    id: id.clone(),
                    name: name.into(),
                }
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
    fn store_voice(&self, voice: &Voice, wav: &[u8]) -> Result<()> {
        let path = self.voice_path(voice.id())?;
        if path.exists() {
            return Err(anyhow!("音声 ID が重複しています"));
        }
        let wav = crate::audio::with_voice(wav, voice)?;
        atomic_bytes(&path, &wav)
    }
    fn voice_path(&self, id: &str) -> Result<PathBuf> {
        check_id(id)?;
        Ok(self.root.join("voices").join(format!("{id}.wav")))
    }
    pub fn chara(&self, id: &str) -> Result<Chara> {
        check_id(id)?;
        let dir = self.root.join("charas").join(id);
        let mut chara: Chara = read_yaml(&dir.join("chara.yaml"))?;
        if chara.id != id {
            return Err(anyhow!("キャラクタ ID が一致しません"));
        }
        if let Some(icon) = &chara.icon {
            if icon != "icon.png" {
                return Err(anyhow!("invalid icon path"));
            }
            chara.icon = Some(format!(
                "data:image/png;base64,{}",
                STANDARD.encode(std::fs::read(dir.join(icon))?)
            ));
        }
        Ok(chara)
    }
    pub fn charas(&self) -> Result<Vec<Chara>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(self.root.join("charas"))? {
            let entry = entry?;
            if let Some(id) = entry.file_name().to_str().filter(|id| valid_id(id)) {
                out.push(self.chara(id)?);
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    pub fn create_chara(&self, input: CharaInput) -> Result<Chara> {
        self.save_chara(new_id(), input)
    }

    pub fn update_chara(&self, id: String, mut input: CharaInput) -> Result<Chara> {
        let existing = self.chara(&id)?;
        if input.color.is_none() {
            input.color = Some(existing.color);
        }
        if input.icon.is_none() {
            input.icon = existing.icon;
        }
        self.save_chara(id, input)
    }

    pub fn delete_chara(&self, id: &str) -> Result<()> {
        self.chara(id)?;
        if self.editor()?.lines.iter().any(|line| line.chara_id == id) {
            return Err(anyhow!("セリフで使用中のキャラクターは削除できません"));
        }
        std::fs::remove_dir_all(self.root.join("charas").join(id))?;
        Ok(())
    }

    fn save_chara(&self, id: String, input: CharaInput) -> Result<Chara> {
        self.save_chara_from(id, input, self)
    }

    fn save_chara_from(&self, id: String, input: CharaInput, source: &Library) -> Result<Chara> {
        check_id(&id)?;
        let chara = input.normalize(id)?;
        let icon = &chara.icon;
        if let Some(value) = icon {
            let data = value
                .strip_prefix("data:image/png;base64,")
                .filter(|data| data.len() <= 150_000)
                .and_then(|data| STANDARD.decode(data).ok());
            if !data.is_some_and(|data| data.starts_with(b"\x89PNG\r\n\x1a\n")) {
                return Err(anyhow!("invalid chara icon"));
            }
        }
        let dir = self.root.join("charas").join(&chara.id);
        let staging = tempfile::Builder::new()
            .prefix(".chara-")
            .tempdir_in(&self.root)?;
        let next = staging.path().join("next");
        let previous = staging.path().join("previous");
        std::fs::create_dir_all(next.join("voices"))?;
        for voice in &chara.voice_ids {
            source.voice_record(voice)?;
            atomic_bytes(
                &next.join("voices").join(format!("{voice}.wav")),
                &source.audio(voice)?,
            )?;
        }
        if dir.join("preview.wav").is_file() {
            std::fs::copy(dir.join("preview.wav"), next.join("preview.wav"))?;
        }
        let mut stored = chara.clone();
        if let Some(icon) = &chara.icon {
            atomic_bytes(
                &next.join("icon.png"),
                &STANDARD.decode(icon.strip_prefix("data:image/png;base64,").unwrap())?,
            )?;
            stored.icon = Some("icon.png".into());
        }
        self.atomic_yaml(&next.join("chara.yaml"), &stored)?;
        let replacing = dir.exists();
        if replacing {
            std::fs::rename(&dir, &previous)?;
        }
        if let Err(error) = std::fs::rename(&next, &dir) {
            if replacing {
                std::fs::rename(&previous, &dir)?;
            }
            return Err(error.into());
        }
        self.chara(&chara.id)
    }
    pub fn takes(&self) -> Result<Vec<Take>> {
        self.editor()?
            .lines
            .into_iter()
            .flat_map(|line| line.takes)
            .map(
                |id| match Self::read_voice_record(&self.voice_path(&id)?, &id)? {
                    Voice::Generated(take) => Ok(*take),
                    Voice::Imported { .. } => {
                        Err(anyhow!("テイクの参照先が生成音声ではありません"))
                    }
                },
            )
            .collect()
    }
    pub fn save_take(&self, take: &Take, wav: &[u8]) -> Result<()> {
        let mut editor = self.editor()?;
        self.store_voice(&Voice::Generated(Box::new(take.clone())), wav)?;
        for line in &mut editor.lines {
            if line.id == take.line_id {
                line.takes.insert(0, take.id.clone());
                take.restore_line(line);
            }
        }
        if let Err(error) = self.write_editor(&editor) {
            std::fs::remove_file(self.voice_path(&take.id)?)?;
            return Err(error);
        }
        Ok(())
    }

    pub fn select_take(&self, line_id: &str, take_id: Option<&str>) -> Result<()> {
        let take = if let Some(id) = take_id {
            let Voice::Generated(take) = Self::read_voice_record(&self.voice_path(id)?, id)? else {
                return Err(anyhow!("生成音声ではありません"));
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
            .ok_or_else(|| anyhow!("セリフが見つかりません"))?;
        if let Some(take) = take {
            if !line.takes.contains(&take.id) {
                return Err(anyhow!("このセリフのテイクではありません"));
            }
            take.restore_line(line);
        } else {
            line.take_id = None;
        }
        self.write_editor(&editor)
    }

    /// Read the WAV header without decoding or loading the PCM into memory.
    pub fn audio_seconds(&self, id: &str) -> Result<f64> {
        let reader = hound::WavReader::open(self.available_voice_path(id)?)?;
        anyhow::ensure!(
            reader.spec().sample_rate > 0,
            "音声のサンプルレートが不正です"
        );
        Ok(reader.duration() as f64 / reader.spec().sample_rate as f64)
    }
    pub fn audio(&self, id: &str) -> Result<Vec<u8>> {
        Ok(std::fs::read(self.available_voice_path(id)?)?)
    }

    pub fn delete_take(&self, id: &str) -> Result<()> {
        let mut editor = self.editor()?;
        for line in &mut editor.lines {
            line.takes.retain(|take| take != id);
            if line.take_id.as_deref() == Some(id) {
                line.take_id = None;
            }
        }
        self.write_editor(&editor)?;
        Ok(std::fs::remove_file(self.voice_path(id)?)?)
    }
}

fn validate_lines(lines: &[crate::domain::Line]) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    let mut takes = std::collections::HashSet::new();
    for line in lines {
        for id in &line.takes {
            check_id(id)?;
            if !takes.insert(id) {
                return Err(anyhow!("テイク ID が重複しています"));
            }
        }
        if line
            .take_id
            .as_ref()
            .is_some_and(|id| !line.takes.contains(id))
        {
            return Err(anyhow!("選択中のテイクがセリフの一覧にありません",));
        }
        if line.id.is_empty() || line.id.len() > 64 {
            return Err(anyhow!("セリフ ID が不正です"));
        }
        if !seen.insert(&line.id) {
            return Err(anyhow!("セリフ ID が重複しています"));
        }
    }
    Ok(())
}

mod transfer;

#[cfg(test)]
mod tests;
