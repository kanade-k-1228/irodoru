//! Window-local application state. Transitions return effects; no GUI or I/O runs here.
use super::protocol::{Event, Snapshot};
use crate::{
    domain::{GenerationSettings, Line},
    project::ProjectStatus,
};
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub enum Field {
    Text,
    Description,
    Prompt,
    Seed,
    Steps,
    Name,
}

#[derive(Default)]
pub struct WorkspaceState {
    pub snapshot: Snapshot,
    pub status: ProjectStatus,
    pub selected: Option<String>,
    pub expanded_line: Option<String>,
    pub chara_locked: bool,
    pub busy: bool,
    pub error: Option<String>,
    pub line_errors: HashMap<String, String>,
    pub running: Option<String>,
    pub stopping: bool,
}

pub enum Effect {
    SyncInputs,
    PlayAudio(Vec<u8>),
    Exported {
        directory: std::path::PathBuf,
        count: usize,
    },
    CloseWindow,
}

impl WorkspaceState {
    pub fn chara_in_use(&self, id: &str) -> bool {
        self.snapshot
            .editor
            .lines
            .iter()
            .any(|line| line.chara_id == id)
    }
    pub fn active_chara(&self) -> Option<&crate::domain::Chara> {
        self.snapshot
            .charas
            .iter()
            .find(|c| c.id == self.snapshot.editor.active_chara)
    }
    fn active_chara_mut(&mut self) -> Option<&mut crate::domain::Chara> {
        self.snapshot
            .charas
            .iter_mut()
            .find(|c| c.id == self.snapshot.editor.active_chara)
    }
    fn selected_line_mut(&mut self) -> Option<&mut Line> {
        let id = self.selected.as_ref()?;
        self.snapshot.editor.lines.iter_mut().find(|r| &r.id == id)
    }
    pub fn settings_value(&self) -> GenerationSettings {
        if let Some(line) = self
            .snapshot
            .editor
            .lines
            .iter()
            .find(|r| Some(&r.id) == self.selected.as_ref())
        {
            return line.settings.clone().unwrap_or_default();
        }
        GenerationSettings::default()
    }
    fn generation_settings_mut(&mut self) -> Option<&mut GenerationSettings> {
        let id = self.selected.as_ref()?;
        let line = self
            .snapshot
            .editor
            .lines
            .iter_mut()
            .find(|line| &line.id == id)?;
        Some(
            line.settings
                .get_or_insert_with(GenerationSettings::default),
        )
    }
    pub fn close_error(&self) -> Option<&'static str> {
        if self.busy {
            Some("処理の完了後に閉じてください。")
        } else {
            None
        }
    }
    pub fn apply(&mut self, event: Event) -> Option<Effect> {
        match event {
            Event::Snapshot(mut snapshot) => {
                let active = &self.snapshot.editor.active_chara;
                if snapshot.charas.iter().any(|c| &c.id == active) {
                    snapshot.editor.active_chara = active.clone();
                }
                self.snapshot = snapshot;
                if !self
                    .snapshot
                    .editor
                    .lines
                    .iter()
                    .any(|r| Some(&r.id) == self.selected.as_ref())
                {
                    self.selected = self.snapshot.editor.lines.first().map(|r| r.id.clone());
                }
                if self.expanded_line != self.selected {
                    self.expanded_line = None;
                }
                if self.active_chara().is_none() {
                    self.snapshot.editor.active_chara = self
                        .snapshot
                        .editor
                        .lines
                        .iter()
                        .find(|line| Some(&line.id) == self.selected.as_ref())
                        .and_then(|line| {
                            self.snapshot.charas.iter().find(|c| c.id == line.chara_id)
                        })
                        .or_else(|| self.snapshot.charas.first())
                        .map(|c| c.id.clone())
                        .unwrap_or_default();
                }
                return Some(Effect::SyncInputs);
            }
            Event::Status(status) => {
                if status.id == self.status.id || self.busy {
                    if let Some(e) = &status.error {
                        self.error = Some(format!("保存できませんでした: {e}"));
                    }
                    if status.id != self.status.id {
                        self.snapshot.editor.active_chara.clear();
                        self.expanded_line = None;
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
            }
            Event::LineStarted(id) => {
                self.line_errors.remove(&id);
                self.running = Some(id);
            }
            Event::LineFailed(id, e) => {
                self.line_errors.insert(id, e);
            }
            Event::CharaCreated(id) => self.snapshot.editor.active_chara = id,
            Event::Audio(bytes) => return Some(Effect::PlayAudio(bytes)),
            Event::Exported { directory, count } => {
                return Some(Effect::Exported { directory, count });
            }
            Event::Close => return Some(Effect::CloseWindow),
        }
        None
    }
}

mod editing;
pub use editing::{GenerationField, LineAction};

#[cfg(test)]
mod tests;
