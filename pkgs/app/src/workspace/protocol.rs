use crate::{
    domain::{AudioSource, Chara, CharaInput, EditorState, SynthesisInput, Take, TakeRequest},
    project,
};
use std::path::PathBuf;

#[derive(Clone, Default)]
pub struct Snapshot {
    pub chara_previews: std::collections::HashSet<String>,
    pub editor: EditorState,
    pub charas: Vec<Chara>,
    pub takes: Vec<Take>,
    pub voices: Vec<crate::domain::Voice>,
}
pub enum Command {
    Open {
        path: PathBuf,
        create: bool,
    },
    NewFromCharas(PathBuf),
    ExportChara {
        input: CharaInput,
        path: PathBuf,
    },
    ImportChara(PathBuf),
    SaveEditor(EditorState),
    Flush {
        close: bool,
    },
    SaveChara {
        id: Option<String>,
        input: CharaInput,
    },
    DeleteChara(String),
    ImportVoices {
        chara_id: String,
        paths: Vec<PathBuf>,
    },
    ImportIcon {
        chara_id: String,
        path: PathBuf,
    },
    DeleteTake(String),
    SelectTake {
        line_id: String,
        take_id: Option<String>,
    },
    Generate(Vec<TakeRequest>),
    Preview {
        chara_id: String,
        input: SynthesisInput,
        description: String,
        voice_ids: Vec<String>,
    },
    Audio(AudioSource),
    ExportLines {
        parent: PathBuf,
        naming: crate::project::ExportNaming,
    },
    Export {
        source: AudioSource,
        path: PathBuf,
    },
}

impl Command {
    /// Passive writes neither block editing nor complete a foreground operation.
    pub fn is_passive(&self) -> bool {
        matches!(
            self,
            Self::SaveEditor(_) | Self::SaveChara { id: Some(_), .. }
        )
    }
}

pub enum Event {
    Snapshot(Snapshot),
    Status(project::ProjectStatus),
    Error(String),
    Done,
    LineStarted(String),
    LineFailed(String, String),
    CharaCreated(String),
    Audio(Vec<u8>),
    Exported { directory: PathBuf, count: usize },
    Close,
}
