use super::Studio;
use gpui_kit::*;

impl Studio {
    pub(super) fn edited(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.autosave();
        self.sync_fields(window, cx);
        cx.notify();
    }
    pub(super) fn move_line(&mut self, after: Option<&str>, cx: &mut Context<Self>) {
        if let Some(index) = self.state.move_line(after) {
            self.autosave();
            self.line_scroll.scroll_to_item(index * 2 + 1);
            cx.notify();
        }
    }
    pub(super) fn select(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.select(id) {
            self.edited(window, cx);
        }
    }
    pub(super) fn add(&mut self, duplicate: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.add_line(duplicate) {
            self.edited(window, cx);
        }
    }
    pub(super) fn generate(&mut self, random: bool, window: &mut Window, cx: &mut Context<Self>) {
        if random {
            match self.state.randomize_selected_line(rand::random()) {
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
        match self.state.generate() {
            Ok(Some(command)) => {
                self.autosave();
                self.send(command);
            }
            Ok(None) => {}
            Err(error) => self.state.error = Some(error.to_string()),
        }
    }
    pub(super) fn create_chara(&mut self, cx: &mut Context<Self>) {
        if let Some(command) = self.state.create_chara() {
            self.send(command);
            cx.notify();
        }
    }
    pub(super) fn preview(&mut self) {
        match self.state.preview() {
            Ok(Some(command)) => {
                self.stop_audio();
                self.send(command);
            }
            Ok(None) => {}
            Err(error) => self.state.error = Some(error.to_string()),
        }
    }
}
