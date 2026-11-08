use super::Studio;
use crate::domain::Character;
use crate::workspace::editor;
use crate::workspace::protocol::Command;
use gpui_kit::*;

impl Studio {
    pub(super) fn select(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.select(id) {
            self.autosave();
            self.sync_fields(window, cx);
            cx.notify();
        }
    }
    pub(super) fn add(&mut self, duplicate: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.busy || self.state.draft.is_some() {
            return;
        }
        if self.state.snapshot.editor.active_character.is_empty() {
            self.state.snapshot.editor.active_character = self
                .state
                .snapshot
                .characters
                .first()
                .map(|c| c.id.clone())
                .unwrap_or_default();
        }
        match editor::insert(
            &mut self.state.snapshot.editor,
            self.state.selected.as_deref(),
            duplicate,
        ) {
            Ok(id) => self.select(id, window, cx),
            Err(e) => self.state.error = Some(e.to_string()),
        }
    }
    pub(super) fn generate(
        &mut self,
        all: bool,
        random: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.busy || self.state.draft.is_some() {
            return;
        }
        if !all {
            self.state.defaults = false;
        }
        if random && let Some(id) = self.state.selected.clone() {
            let previous = self
                .state
                .snapshot
                .takes
                .iter()
                .find(|t| t.row_id == id)
                .map(|t| t.seed);
            if let Err(error) = editor::randomize_row(
                &mut self.state.snapshot.editor,
                &id,
                previous,
                rand::random(),
            ) {
                self.state.error = Some(error.to_string());
                return;
            }
            self.autosave();
            self.sync_fields(window, cx);
        }

        let rows = if all {
            self.state.snapshot.editor.rows.clone()
        } else {
            self.state
                .snapshot
                .editor
                .rows
                .iter()
                .filter(|r| Some(&r.id) == self.state.selected.as_ref())
                .cloned()
                .collect()
        };
        match editor::requests(
            &rows,
            &self.state.snapshot.characters,
            &self.state.snapshot.editor.preferences,
        ) {
            Ok(requests) if !requests.is_empty() => {
                self.autosave();
                self.send(Command::Generate(requests), true);
            }
            Ok(_) => {}
            Err(e) => self.state.error = Some(e.to_string()),
        }
    }
    pub(super) fn edit_character(
        &mut self,
        character: Option<Character>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.edit_character(character) {
            self.sync_fields(window, cx);
            cx.notify();
        }
    }
    pub(super) fn preview(&mut self) {
        if let Some(d) = &self.state.draft {
            match editor::speech(d.input.sample_text.clone().unwrap_or_default(), &d.settings) {
                Ok(input) => self.send(
                    Command::Preview {
                        input,
                        description: d.input.description.clone(),
                        voice: d.input.voice.clone(),
                    },
                    true,
                ),
                Err(e) => self.state.error = Some(e.to_string()),
            }
        }
    }
    pub(super) fn discard_character(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.playback = None;
        self.audio_id = None;
        if let Some(draft) = self.state.draft.take()
            && let Some(id) = draft.preview
        {
            self.send(Command::RemovePreview(id), false);
        }
        self.sync_fields(window, cx);
        cx.notify();
    }

    pub(super) fn remove_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.busy || self.state.draft.is_some() {
            return;
        }
        if let Some(id) = self.state.selected.clone() {
            self.state.selected = editor::remove(&mut self.state.snapshot.editor, &id);
            self.state.snapshot.takes.retain(|t| t.row_id != id);
            self.autosave();
            self.sync_fields(window, cx);
        }
        cx.notify();
    }
}
