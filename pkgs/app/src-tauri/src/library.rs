use std::path::{Path as FsPath, PathBuf};

use anyhow::Result;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::AppError;

pub const DEFAULT_CHARACTER_COLOR: &str = "#41a2ec";
pub const DEFAULT_SAMPLE_TEXT: &str = "こんにちは。よろしくお願いします。";
fn default_character_color() -> String {
    DEFAULT_CHARACTER_COLOR.into()
}
fn default_sample_text() -> String {
    DEFAULT_SAMPLE_TEXT.into()
}

#[derive(Clone, Deserialize, Serialize, TS)]
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
    #[ts(optional)]
    pub icon: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CharacterInput {
    pub name: String,
    pub description: String,
    pub voice: Option<String>,
    pub color: Option<String>,
    pub sample_text: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub icon: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, TS)]
pub struct Take {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
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
    #[ts(type = "string")]
    pub seed: u64,
    pub num_steps: usize,
    #[ts(type = "number")]
    pub elapsed_ms: u64,
    pub audio_seconds: f64,
    #[ts(type = "number")]
    pub created_at: u64,
}

pub struct Library {
    root: PathBuf,
}

pub fn new_id() -> String {
    format!("{:032x}", rand::random::<u128>())
}
fn valid_id(id: &str) -> bool {
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
fn read_json<T: serde::de::DeserializeOwned>(path: &FsPath) -> Result<T, AppError> {
    let bytes = std::fs::read(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            AppError::not_found("not found")
        } else {
            AppError::internal(e)
        }
    })?;
    serde_json::from_slice(&bytes).map_err(AppError::internal)
}

impl Library {
    pub fn open(root: &FsPath) -> Result<Self> {
        std::fs::create_dir_all(root.join("characters"))?;
        std::fs::create_dir_all(root.join("takes"))?;
        std::fs::create_dir_all(root.join("voices"))?;
        Ok(Self {
            root: root.to_owned(),
        })
    }
    fn atomic_json(&self, path: &FsPath, value: &impl Serialize) -> Result<(), AppError> {
        use std::io::Write;
        let mut temp =
            tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(AppError::internal)?;
        serde_json::to_writer(&mut temp, value).map_err(AppError::internal)?;
        temp.flush().map_err(AppError::internal)?;
        temp.as_file().sync_all().map_err(AppError::internal)?;
        temp.persist(path).map_err(AppError::internal)?;
        Ok(())
    }
    pub fn editor(&self) -> Result<crate::types::EditorState, AppError> {
        let path = self.root.join("editor.json");
        if !path.exists() {
            return Ok(crate::types::EditorState::default());
        }
        read_json(&path)
    }
    pub fn save_editor(&self, state: &crate::types::EditorState) -> Result<(), AppError> {
        self.atomic_json(&self.root.join("editor.json"), state)
    }
    pub fn voices(&self) -> Result<Vec<String>, AppError> {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(self.root.join("voices")).map_err(AppError::internal)? {
            let path = entry.map_err(AppError::internal)?.path();
            if path.extension().is_some_and(|e| e == "wav") {
                names.push(path.file_stem().unwrap().to_string_lossy().into_owned());
            }
        }
        names.sort();
        Ok(names)
    }
    fn voice_path(&self, name: &str) -> Result<PathBuf, AppError> {
        if name.is_empty()
            || name.chars().count() > 64
            || name.starts_with('.')
            || name.chars().any(|c| {
                c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
            })
        {
            return Err(AppError::invalid("invalid reference voice name"));
        }
        Ok(self.root.join("voices").join(format!("{name}.wav")))
    }
    pub fn voice(&self, name: &str) -> Result<Vec<u8>, AppError> {
        std::fs::read(self.voice_path(name)?).map_err(AppError::internal)
    }
    pub fn save_voice(&self, name: &str, wav: &[u8]) -> Result<(), AppError> {
        use std::io::Write;
        crate::generation::validate_wav(wav).map_err(AppError::invalid)?;
        let path = self.voice_path(name)?;
        let mut temp = tempfile::NamedTempFile::new_in(self.root.join("voices"))
            .map_err(AppError::internal)?;
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
        read_json(&self.root.join("characters").join(format!("{id}.json")))
    }
    pub fn characters(&self) -> Result<Vec<Character>, AppError> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(self.root.join("characters")).map_err(AppError::internal)? {
            let path = entry.map_err(AppError::internal)?.path();
            if path.extension().is_some_and(|e| e == "json")
                && path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(valid_id)
            {
                out.push(read_json::<Character>(&path)?);
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
        std::fs::remove_file(self.root.join("characters").join(format!("{id}.json")))
            .map_err(AppError::internal)
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
        let dir = self.root.join("characters");
        self.atomic_json(&dir.join(format!("{}.json", character.id)), &character)?;
        Ok(character)
    }
    pub fn takes(&self) -> Result<Vec<Take>, AppError> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(self.root.join("takes")).map_err(AppError::internal)? {
            let entry = entry.map_err(AppError::internal)?;
            if entry.file_name().to_str().is_some_and(valid_id) {
                out.push(read_json::<Take>(&entry.path().join("meta.json"))?);
            }
        }
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
        Ok(out)
    }
    pub fn save_take(&self, take: &Take, wav: &[u8]) -> Result<(), AppError> {
        check_id(&take.id)?;
        let dir = self.root.join("takes");
        let temp = dir.join(format!(".{}", take.id));
        let write = || -> Result<()> {
            std::fs::create_dir(&temp)?;
            std::fs::write(temp.join("audio.wav"), wav)?;
            std::fs::write(temp.join("meta.json"), serde_json::to_vec(take)?)?;
            std::fs::rename(&temp, dir.join(&take.id))?;
            Ok(())
        };
        if let Err(e) = write() {
            let _ = std::fs::remove_dir_all(temp);
            return Err(AppError::internal(e));
        }
        Ok(())
    }

    fn take_dir(&self, id: &str) -> Result<PathBuf, AppError> {
        check_id(id)?;
        let dir = self.root.join("takes").join(id);
        let _: Take = read_json(&dir.join("meta.json"))?;
        Ok(dir)
    }

    pub fn export_filename(&self, id: &str) -> Result<String, AppError> {
        let take: Take = read_json(&self.take_dir(id)?.join("meta.json"))?;
        Ok(take
            .filename
            .filter(|name| valid_wav_filename(name))
            .unwrap_or_else(|| format!("irodoru-{id}.wav")))
    }
    pub fn audio(&self, id: &str) -> Result<Vec<u8>, AppError> {
        std::fs::read(self.take_dir(id)?.join("audio.wav")).map_err(AppError::internal)
    }

    pub fn delete_take(&self, id: &str) -> Result<(), AppError> {
        std::fs::remove_dir_all(self.take_dir(id)?).map_err(AppError::internal)
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
    fn character_presentation_and_selected_take_survive_reopening() {
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
        assert_eq!(
            reopened.editor().unwrap().rows[0].take_id.as_deref(),
            Some("chosen-take")
        );
        assert!(!valid_wav_filename("../../outside.wav"));
        assert!(valid_wav_filename(&format!("{}.wav", "a".repeat(64))));
    }

    #[test]
    fn takes_survive_reopening_and_character_edits_without_overwriting_audio() {
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
        assert!(library.save_take(&first, b"replacement").is_err());
        library
            .update_character(character.id.clone(), character_input("編集後"))
            .unwrap();
        let reopened = Library::open(&library.root).unwrap();
        assert_eq!(reopened.characters().unwrap()[0].name, "編集後");
        let takes = reopened.takes().unwrap();
        assert_eq!(takes.len(), 2);
        assert_eq!(takes[0].id, second.id);
        assert_eq!(takes[1].character_name, "さくら");
        for (take, bytes) in [
            (&first, b"first wav".as_slice()),
            (&second, b"second wav".as_slice()),
        ] {
            assert_eq!(reopened.audio(&take.id).unwrap(), bytes);
        }
        reopened.delete_character(&character.id).unwrap();
        assert_eq!(reopened.takes().unwrap().len(), 2);
        assert!(matches!(
            reopened.update_character(character.id.clone(), character_input("missing")),
            Err(AppError::NotFound(_))
        ));
        reopened.delete_take(&first.id).unwrap();
        assert!(matches!(
            reopened.audio(&first.id),
            Err(AppError::NotFound(_))
        ));
        assert_eq!(reopened.audio(&second.id).unwrap(), b"second wav");
    }

    #[test]
    fn incomplete_writes_are_hidden_and_invalid_ids_and_names_are_rejected() {
        let test = TestLibrary::new();
        let library = &test.0;
        std::fs::create_dir(library.root.join("takes").join(format!(".{}", new_id()))).unwrap();
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
