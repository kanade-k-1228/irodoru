//! Window-local application state. Transitions return effects; no GUI or I/O runs here.
use super::protocol::{Event, Snapshot};
use crate::{
    domain::{CharacterInput, GenerationSettings, ScriptRow},
    project::ProjectStatus,
};
use std::collections::HashMap;

pub struct Draft {
    pub id: Option<String>,
    pub input: CharacterInput,
    pub preview: Option<String>,
    pub settings: GenerationSettings,
    pub original: CharacterInput,
}
#[derive(Clone, Copy)]
pub enum Field {
    Text,
    Description,
    Prompt,
    Seed,
    Steps,
    Name,
    Sample,
}

#[derive(Default)]
pub struct WorkspaceState {
    pub snapshot: Snapshot,
    pub status: ProjectStatus,
    pub selected: Option<String>,
    pub draft: Option<Draft>,
    pub busy: bool,
    pub saving_draft: bool,
    pub error: Option<String>,
    pub row_errors: HashMap<String, String>,
    pub running: Option<String>,
    pub stopping: bool,
}

pub enum Effect {
    SyncInputs,
    RemovePreview(String),
    PlayAudio(Vec<u8>),
    Exported {
        directory: std::path::PathBuf,
        count: usize,
    },
    CloseWindow,
}

impl WorkspaceState {
    fn selected_row_mut(&mut self) -> Option<&mut ScriptRow> {
        let id = self.selected.as_ref()?;
        self.snapshot.editor.rows.iter_mut().find(|r| &r.id == id)
    }
    pub fn settings_value(&self) -> GenerationSettings {
        if let Some(draft) = &self.draft {
            return draft.settings.clone();
        }
        if let Some(row) = self
            .snapshot
            .editor
            .rows
            .iter()
            .find(|r| Some(&r.id) == self.selected.as_ref())
        {
            return row
                .settings
                .clone()
                .unwrap_or_else(|| self.snapshot.editor.preferences.clone());
        }
        self.snapshot.editor.preferences.clone()
    }
    fn generation_settings_mut(&mut self) -> &mut GenerationSettings {
        if let Some(draft) = &mut self.draft {
            return &mut draft.settings;
        }
        if let Some(index) = self
            .snapshot
            .editor
            .rows
            .iter()
            .position(|r| Some(&r.id) == self.selected.as_ref())
        {
            let defaults = self.snapshot.editor.preferences.clone();
            return self.snapshot.editor.rows[index]
                .settings
                .get_or_insert(defaults);
        }
        &mut self.snapshot.editor.preferences
    }
    pub fn close_error(&self) -> Option<&'static str> {
        if self.busy {
            Some("処理の完了後に閉じてください。")
        } else if self.draft.is_some() {
            Some("キャラクターの編集を保存またはキャンセルしてから閉じてください。")
        } else {
            None
        }
    }
    pub fn apply(&mut self, event: Event) -> Option<Effect> {
        match event {
            Event::Snapshot(snapshot) => {
                self.snapshot = snapshot;
                if self.saving_draft {
                    self.saving_draft = false;
                    self.draft = None;
                }
                if !self
                    .snapshot
                    .editor
                    .rows
                    .iter()
                    .any(|r| Some(&r.id) == self.selected.as_ref())
                {
                    self.selected = self.snapshot.editor.rows.first().map(|r| r.id.clone());
                }
                return Some(Effect::SyncInputs);
            }
            Event::Status(status) => {
                if status.id == self.status.id || self.busy {
                    if let Some(e) = &status.error {
                        self.error = Some(format!("保存できませんでした: {e}"));
                    }
                    self.status = status;
                }
            }
            Event::Done => {
                self.busy = false;
                self.running = None;
                self.stopping = false;
            }
            Event::Error(e) => {
                self.error = Some(e);
                self.saving_draft = false;
            }
            Event::RowStarted(id) => {
                self.row_errors.remove(&id);
                self.running = Some(id);
            }
            Event::RowFailed(id, e) => {
                self.row_errors.insert(id, e);
            }
            Event::Preview(id) => {
                if let Some(d) = &mut self.draft
                    && let Some(old) = d.preview.replace(id)
                {
                    return Some(Effect::RemovePreview(old));
                }
            }
            Event::Voice(voice) => {
                if let Some(d) = &mut self.draft
                    && !d.input.voice_ids.contains(&voice)
                {
                    d.input.voice_ids.push(voice);
                }
            }
            Event::Icon(icon) => {
                if let Some(d) = &mut self.draft {
                    d.input.icon = Some(icon);
                }
            }
            Event::Audio(bytes) => return Some(Effect::PlayAudio(bytes)),
            Event::Exported { directory, count } => {
                return Some(Effect::Exported { directory, count });
            }
            Event::Close => return Some(Effect::CloseWindow),
        }
        None
    }
}
#[cfg(test)]
mod tests;

mod editing;
pub use editing::{GenerationField, RowAction};
