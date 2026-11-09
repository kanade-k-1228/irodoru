use super::Studio;
use crate::domain::Character;
use gpui_kit::*;

impl Studio {
    pub(super) fn edited(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.autosave();
        self.sync_fields(window, cx);
        cx.notify();
    }
    pub(super) fn move_row(&mut self, after: Option<&str>, cx: &mut Context<Self>) {
        if let Some(index) = self.state.move_row(after) {
            self.autosave();
            self.script_scroll.scroll_to_item(index * 2 + 1);
            cx.notify();
        }
    }
    pub(super) fn select(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.select(id) {
            self.edited(window, cx);
        }
    }
    pub(super) fn add(&mut self, duplicate: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.add_row(duplicate) {
            self.edited(window, cx);
        }
    }
    pub(super) fn generate(
        &mut self,
        all: bool,
        random: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if random {
            match self.state.randomize_selected_row(rand::random()) {
                Ok(true) => {
                    self.autosave();
                    self.sync_fields(window, cx);
                }
                Ok(false) => {}
                Err(error) => {
                    self.state.error = Some(error.to_string());
                    return;
                }
            }
        }
        match self.state.generate(all) {
            Ok(Some(command)) => {
                self.autosave();
                self.send(command);
            }
            Ok(None) => {}
            Err(error) => self.state.error = Some(error.to_string()),
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
        match self.state.preview() {
            Ok(Some(command)) => self.send(command),
            Ok(None) => {}
            Err(error) => self.state.error = Some(error.to_string()),
        }
    }
    pub(super) fn discard_character(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.playback = None;
        self.audio_id = None;
        if let Some(command) = self.state.discard_character() {
            self.send(command);
        }
        self.sync_fields(window, cx);
        cx.notify();
    }
    pub(super) fn remove_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.remove_row() {
            self.edited(window, cx);
        }
    }
}
