use super::Session;
use crate::domain::{EditorState, Take};
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
pub enum ExportNaming {
    Number,
    Character,
    Dialogue,
    CharacterDialogue,
}

impl ExportNaming {
    pub const ALL: [Self; 4] = [
        Self::Number,
        Self::Character,
        Self::Dialogue,
        Self::CharacterDialogue,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Number => "連番 — 001.wav",
            Self::Character => "連番＋キャラクター名 — 001_さくら.wav",
            Self::Dialogue => "連番＋セリフ — 001_こんにちは.wav",
            Self::CharacterDialogue => "連番＋キャラクター名＋セリフ — 001_さくら_こんにちは.wav",
        }
    }

    fn filename(self, index: usize, take: &Take) -> String {
        let mut parts = vec![format!("{:03}", index + 1)];
        if matches!(self, Self::Character | Self::CharacterDialogue) {
            parts.push(filename_part(&take.character_name, "キャラクター"));
        }
        if matches!(self, Self::Dialogue | Self::CharacterDialogue) {
            parts.push(filename_part(&take.conditions.text, "セリフ"));
        }
        format!("{}.wav", parts.join("_"))
    }
}

fn filename_part(value: &str, fallback: &str) -> String {
    let mut result = String::new();
    for ch in value.trim().chars() {
        let ch = if ch.is_control() || "/\\:*?\"<>|".contains(ch) {
            '_'
        } else {
            ch
        };
        if result.len() + ch.len_utf8() > 90 {
            break;
        }
        result.push(ch);
    }
    let result = result.trim_matches(|ch: char| ch == '.' || ch.is_whitespace());
    if result.is_empty() {
        fallback.into()
    } else {
        result.into()
    }
}

pub(crate) fn export_rows<'a>(editor: &EditorState, takes: &'a [Take]) -> Result<Vec<&'a Take>> {
    if editor.rows.is_empty() {
        bail!("書き出すセリフがありません");
    }
    let mut selected = Vec::new();
    let mut missing = Vec::new();
    for (index, row) in editor.rows.iter().enumerate() {
        match takes
            .iter()
            .find(|take| row.take_id.as_ref() == Some(&take.id) && take.row_id == row.id)
        {
            Some(take) => selected.push(take),
            None => missing.push((index + 1).to_string()),
        }
    }
    if !missing.is_empty() {
        bail!(
            "{}行目の音声が選択されていません。テイクを生成・選択してから書き出してください。",
            missing.join("、")
        );
    }
    Ok(selected)
}

impl Session {
    pub(crate) fn export_script(
        &self,
        parent: &Path,
        naming: ExportNaming,
    ) -> Result<(PathBuf, usize)> {
        let takes = self.library.takes()?;
        let selected = export_rows(&self.library.editor()?, &takes)?;
        let directory = (1..)
            .find_map(|index| {
                let name = if index == 1 {
                    "irodoru-export".into()
                } else {
                    format!("irodoru-export-{index}")
                };
                let path = parent.join(name);
                match std::fs::create_dir(&path) {
                    Ok(()) => Some(Ok(path)),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                    Err(error) => Some(Err(error)),
                }
            })
            .expect("unbounded directory search")?;
        let result = (|| -> Result<()> {
            for (index, take) in selected.iter().enumerate() {
                let wav = self.library.audio(&take.id)?;
                crate::files::atomic_bytes(&directory.join(naming.filename(index, take)), &wav)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            let _ = std::fs::remove_dir_all(&directory);
            return Err(error);
        }
        Ok((directory, selected.len()))
    }
}
