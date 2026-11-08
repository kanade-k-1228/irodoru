use crate::{
    domain::{Character, CharacterInput, EditorState, SynthesisInput, Take, TakeRequest},
    project,
};
use std::path::PathBuf;

#[derive(Clone, Default)]
pub struct Snapshot {
    pub character_previews: std::collections::HashSet<String>,
    pub editor: EditorState,
    pub characters: Vec<Character>,
    pub takes: Vec<Take>,
    pub voices: Vec<String>,
}
pub enum Command {
    ExportCharacter {
        id: String,
        path: PathBuf,
    },
    Open {
        path: PathBuf,
        create: bool,
    },
    SaveEditor(EditorState),
    Flush {
        close: bool,
    },
    SaveCharacter {
        id: Option<String>,
        input: CharacterInput,
        preview: Option<String>,
    },
    DeleteCharacter(String),
    ImportVoice(PathBuf),
    ImportIcon(PathBuf),
    DeleteTake(String),
    Generate(Vec<TakeRequest>),
    Preview {
        input: SynthesisInput,
        description: String,
        voice: Option<String>,
    },
    RemovePreview(String),
    Audio {
        id: String,
        preview: bool,
    },
    CharacterAudio(String),
    Export {
        id: String,
        preview: bool,
        path: PathBuf,
    },
}

impl Command {
    /// Passive writes neither block editing nor complete a foreground operation.
    pub fn is_passive(&self) -> bool {
        matches!(self, Self::SaveEditor(_) | Self::RemovePreview(_))
    }
}

pub enum Event {
    Snapshot(Snapshot),
    Status(project::ProjectStatus),
    Error(String),
    Done,
    RowStarted(String),
    RowFailed(String, String),
    Preview(String),
    Voice(String),
    Icon(String),
    Audio(Vec<u8>),
    Close,
}
