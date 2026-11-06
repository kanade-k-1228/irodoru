use std::path::{Path as FsPath, PathBuf};

use anyhow::Result;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const DEFAULT_CHARACTER_COLOR: &str = "#41a2ec";
pub const DEFAULT_SAMPLE_TEXT: &str = "こんにちは。よろしくお願いします。";
fn default_character_color() -> String {
    DEFAULT_CHARACTER_COLOR.into()
}
fn default_sample_text() -> String {
    DEFAULT_SAMPLE_TEXT.into()
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Character {
    pub id: String,
    pub name: String,
    pub description: String,
    pub voice: Option<String>,
    #[serde(default = "default_character_color")]
    pub color: String,
    #[serde(default = "default_sample_text")]
    pub sample_text: String,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterInput {
    pub name: String,
    pub description: String,
    pub voice: Option<String>,
    pub color: Option<String>,
    pub sample_text: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Take {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    pub id: String,
    pub row_id: String,
    pub character_id: Option<String>,
    pub character_name: String,
    pub description: String,
    #[serde(default)]
    pub additional_prompt: String,
    pub voice: Option<String>,
    pub text: String,
    pub model: String,
    #[serde(with = "seed_string")]
    pub seed: u64,
    pub num_steps: usize,

    pub elapsed_ms: u64,
    pub audio_seconds: f64,

    pub created_at: u64,
}

pub struct Library {
    root: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct ProjectDocument {
    version: u32,
    #[serde(flatten)]
    editor: crate::types::EditorState,
}

pub(crate) fn atomic_bytes(path: &FsPath, bytes: &[u8]) -> Result<(), AppError> {
    use std::io::Write;
    let mut temp =
        tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(AppError::internal)?;
    temp.write_all(bytes).map_err(AppError::internal)?;
    temp.as_file().sync_all().map_err(AppError::internal)?;
    temp.persist(path).map_err(AppError::internal)?;
    Ok(())
}

pub fn new_id() -> String {
    format!("{:032x}", rand::random::<u128>())
}
pub(crate) fn valid_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
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
        use std::io::Write;
        let mut temp =
            tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(AppError::internal)?;
        serde_yaml::to_writer(&mut temp, value).map_err(AppError::internal)?;
        temp.flush().map_err(AppError::internal)?;
        temp.as_file().sync_all().map_err(AppError::internal)?;
        temp.persist(path).map_err(AppError::internal)?;
        Ok(())
    }
    pub fn root(&self) -> &FsPath {
        &self.root
    }
    pub fn editor(&self) -> Result<crate::types::EditorState, AppError> {
        let path = self.root.join("project.yaml");
        if !path.exists() {
            return Ok(crate::types::EditorState::default());
        }
        let project: ProjectDocument = read_yaml(&path)?;
        if project.version != 1 {
            return Err(AppError::invalid("未対応のプロジェクト形式です"));
        }
        Ok(project.editor)
    }
    pub fn save_editor(&self, state: &crate::types::EditorState) -> Result<(), AppError> {
        let takes = self.takes()?;
        let mut editor = state.clone();
        // Audio ownership is authoritative in the backend, including while UI saves are queued.
        for row in &mut editor.rows {
            row.take_id = takes
                .iter()
                .find(|take| take.row_id == row.id)
                .map(|take| take.id.clone());
        }
        self.write_editor(&editor)?;
        for take in takes {
            if !editor.rows.iter().any(|row| row.id == take.row_id) {
                self.delete_take(&take.id)?;
            }
        }
        Ok(())
    }
    fn write_editor(&self, editor: &crate::types::EditorState) -> Result<(), AppError> {
        self.atomic_yaml(
            &self.root.join("project.yaml"),
            &ProjectDocument {
                version: 1,
                editor: editor.clone(),
            },
        )
    }
    pub fn voices(&self) -> Result<Vec<String>, AppError> {
        let mut names: Vec<_> = self
            .characters()?
            .into_iter()
            .filter_map(|c| c.voice)
            .collect();
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
        crate::generation::validate_wav(wav).map_err(AppError::invalid)?;
        let path = self.voice_path(name)?;
        let mut temp =
            tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(AppError::internal)?;
        temp.write_all(wav).map_err(AppError::internal)?;
        temp.as_file().sync_all().map_err(AppError::internal)?;
        temp.persist_noclobber(path).map_err(AppError::internal)?;
        Ok(())
    }
    pub fn audio_path(&self, id: &str) -> Result<PathBuf, AppError> {
        Ok(self.take_dir(id)?.join("audio.wav"))
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
        let name = input.name.trim().to_owned();
        if name.is_empty() || name.chars().count() > 64 || input.description.chars().count() > 2000
        {
            return Err(AppError::invalid(
                "name must be 1–64 characters; description at most 2000",
            ));
        }
        if let Some(voice) = input.voice.as_ref().filter(|s| !s.is_empty()) {
            self.voice(voice)?;
        }
        let color = input.color.unwrap_or_else(default_character_color);
        if color.len() != 7
            || !color.starts_with('#')
            || !color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
        {
            return Err(AppError::invalid("color must be #RRGGBB"));
        }
        let sample_text = input.sample_text.unwrap_or_else(default_sample_text);
        if sample_text.chars().count() > 1000 {
            return Err(AppError::invalid(
                "sample_text must be at most 1000 characters",
            ));
        }
        let icon = input.icon.filter(|icon| !icon.is_empty());
        if let Some(value) = &icon {
            let data = value
                .strip_prefix("data:image/png;base64,")
                .filter(|data| data.len() <= 150_000)
                .and_then(|data| STANDARD.decode(data).ok());
            if !data.is_some_and(|data| data.starts_with(b"\x89PNG\r\n\x1a\n")) {
                return Err(AppError::invalid("invalid character icon"));
            }
        }
        let character = Character {
            icon,
            color: color.to_ascii_lowercase(),
            sample_text,
            id,
            name,
            description: input.description.trim().into(),
            voice: input.voice.filter(|v| !v.is_empty()),
        };
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
                row.take_id = Some(take.id.clone());
            }
        }
        self.write_editor(&editor)?;
        for previous in self.takes()? {
            if previous.row_id == take.row_id && previous.id != take.id {
                self.delete_take(&previous.id)?;
            }
        }
        Ok(())
    }

    fn take_dir(&self, id: &str) -> Result<PathBuf, AppError> {
        check_id(id)?;
        let dir = self.root.join("voices").join(id);
        let _: Take = read_yaml(&dir.join("meta.yaml"))?;
        Ok(dir)
    }

    pub fn export_filename(&self, id: &str) -> Result<String, AppError> {
        let take: Take = read_yaml(&self.take_dir(id)?.join("meta.yaml"))?;
        Ok(take
            .filename
            .filter(|name| valid_wav_filename(name))
            .unwrap_or_else(|| format!("irodoru-{id}.wav")))
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
mod tests {
    use super::*;

    struct TestLibrary(Library);
    impl TestLibrary {
        fn new() -> Self {
            Self(
                Library::open(&std::env::temp_dir().join(format!("irodoru-library-{}", new_id())))
                    .unwrap(),
            )
        }
    }
    impl Drop for TestLibrary {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0.root);
        }
    }
    fn character_input(name: &str) -> CharacterInput {
        CharacterInput {
            icon: None,
            color: None,
            sample_text: None,
            name: name.into(),
            description: "落ち着いた声".into(),
            voice: None,
        }
    }

    #[test]
    fn character_icon_persists_and_can_be_removed() {
        let test = TestLibrary::new();
        let mut input = character_input("さくら");
        let icon = "data:image/png;base64,iVBORw0KGgo=";
        input.icon = Some(icon.into());
        let character = test.0.create_character(input).unwrap();
        assert_eq!(
            test.0.character(&character.id).unwrap().icon.as_deref(),
            Some(icon)
        );
        let unchanged = test
            .0
            .update_character(character.id.clone(), character_input("さくら"))
            .unwrap();
        assert_eq!(unchanged.icon.as_deref(), Some(icon));
        let mut removed = character_input("さくら");
        removed.icon = Some(String::new());
        assert!(
            test.0
                .update_character(character.id, removed)
                .unwrap()
                .icon
                .is_none()
        );
        let mut invalid = character_input("不正");
        invalid.icon = Some("data:image/svg+xml;base64,AAAA".into());
        assert!(test.0.create_character(invalid).is_err());
    }

    #[test]
    fn character_presentation_survives_and_missing_take_selection_is_cleared() {
        let test = TestLibrary::new();
        let mut input = character_input("さくら");
        input.color = Some("#ABCDEF".into());
        input.sample_text = Some("サンプル文".into());
        let character = test.0.create_character(input).unwrap();
        let edited = test
            .0
            .update_character(character.id.clone(), character_input("更新後"))
            .unwrap();
        assert_eq!(edited.color, "#abcdef");
        assert_eq!(edited.sample_text, "サンプル文");
        let mut invalid = character_input("不正");
        invalid.color = Some("red".into());
        assert!(
            test.0
                .update_character(character.id.clone(), invalid)
                .is_err()
        );
        let mut editor = crate::types::EditorState::default();
        editor.rows.push(crate::types::ScriptRow {
            id: "row".into(),
            text: "本文".into(),
            character_id: character.id.clone(),
            prompt: None,
            settings: None,
            take_id: Some("chosen-take".into()),
        });
        test.0.save_editor(&editor).unwrap();
        let reopened = Library::open(&test.0.root).unwrap();
        assert_eq!(reopened.character(&character.id).unwrap().name, "更新後");
        assert_eq!(reopened.editor().unwrap().rows[0].take_id.as_deref(), None);
        assert!(!valid_wav_filename("../../outside.wav"));
        assert!(valid_wav_filename(&format!("{}.wav", "a".repeat(64))));
    }

    #[test]
    fn regeneration_replaces_previous_audio_and_survives_character_edits() {
        let test = TestLibrary::new();
        let library = &test.0;
        let character = library.create_character(character_input("さくら")).unwrap();
        let first = Take {
            filename: None,
            id: new_id(),
            row_id: "same-row".into(),
            character_id: Some(character.id.clone()),
            character_name: character.name.clone(),
            description: character.description.clone(),
            additional_prompt: String::new(),
            voice: None,
            text: "こんにちは".into(),
            model: "mf".into(),
            seed: 42,
            num_steps: 4,
            elapsed_ms: 100,
            audio_seconds: 1.0,
            created_at: 1,
        };
        library.save_take(&first, b"first wav").unwrap();
        let mut old_metadata = serde_json::to_value(&first).unwrap();
        old_metadata
            .as_object_mut()
            .unwrap()
            .remove("additional_prompt");
        let old_take: Take = serde_json::from_value(old_metadata).unwrap();
        assert!(old_take.additional_prompt.is_empty());
        let second = Take {
            filename: None,
            id: new_id(),
            created_at: 2,
            ..first.clone()
        };
        library.save_take(&second, b"second wav").unwrap();
        assert!(library.save_take(&second, b"replacement").is_err());
        library
            .update_character(character.id.clone(), character_input("編集後"))
            .unwrap();
        let reopened = Library::open(&library.root).unwrap();
        assert_eq!(reopened.characters().unwrap()[0].name, "編集後");
        let takes = reopened.takes().unwrap();
        assert_eq!(takes.len(), 1);
        assert_eq!(takes[0].id, second.id);
        assert_eq!(takes[0].character_name, "さくら");
        assert!(reopened.audio(&first.id).is_err());
        assert_eq!(reopened.audio(&second.id).unwrap(), b"second wav");
        reopened.delete_character(&character.id).unwrap();
        assert_eq!(reopened.takes().unwrap().len(), 1);
        assert!(matches!(
            reopened.update_character(character.id.clone(), character_input("missing")),
            Err(AppError::NotFound(_))
        ));
        reopened.delete_take(&second.id).unwrap();
        assert!(matches!(
            reopened.audio(&first.id),
            Err(AppError::NotFound(_))
        ));
        assert!(reopened.takes().unwrap().is_empty());
    }

    #[test]
    fn incomplete_writes_are_hidden_and_invalid_ids_and_names_are_rejected() {
        let test = TestLibrary::new();
        let library = &test.0;
        std::fs::create_dir(library.root.join("voices").join(format!(".{}", new_id()))).unwrap();
        std::fs::write(
            library
                .root
                .join("characters")
                .join(format!(".{}.tmp", new_id())),
            b"partial",
        )
        .unwrap();
        assert!(library.takes().unwrap().is_empty());
        assert!(library.characters().unwrap().is_empty());
        for invalid in ["../outside", "", "012345678901234567890123456789012g"] {
            assert!(library.character(invalid).is_err());
            assert!(matches!(library.audio(invalid), Err(AppError::NotFound(_))));
            assert!(matches!(
                library.delete_take(invalid),
                Err(AppError::NotFound(_))
            ));
            assert!(
                library
                    .save_character(invalid.into(), character_input("test"))
                    .is_err()
            );
        }
        for invalid in [" ".to_owned(), "あ".repeat(65)] {
            assert!(
                library
                    .save_character(new_id(), character_input(&invalid))
                    .is_err()
            );
        }
        assert!(
            library
                .save_character(new_id(), character_input(&"あ".repeat(64)))
                .is_ok()
        );
    }
}

mod seed_string {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(seed: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&seed.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

pub fn valid_wav_filename(name: &str) -> bool {
    name.len() == 68
        && name.ends_with(".wav")
        && name.as_bytes()[..64].iter().all(u8::is_ascii_hexdigit)
}
