use super::Studio;
use crate::audio::Playback;
use crate::workspace::protocol::Command;
use crate::workspace::state::Effect;
use gpui_kit::*;

impl Studio {
    pub(super) fn send(&mut self, command: Command) {
        let busy = !command.is_passive();
        if let Some(service) = &self.service {
            if busy && self.state.status.id.is_some() {
                for chara in &self.state.snapshot.charas {
                    if let Err(error) = service.send(Command::SaveChara {
                        id: Some(chara.id.clone()),
                        input: chara.clone().into(),
                    }) {
                        self.state.error = Some(error.to_string());
                        return;
                    }
                }
            }
            match service.send(command) {
                Ok(()) => {
                    if busy {
                        self.state.busy = true;
                        self.state.error = None;
                    }
                }
                Err(e) => self.state.error = Some(e.to_string()),
            }
        }
    }
    pub(super) fn autosave_chara(&mut self) {
        if let Some(command) = self.state.save_chara() {
            self.state.status.state = crate::project::SaveState::Saving;
            self.state.error = None;
            self.send(command);
        }
    }
    pub(super) fn autosave(&mut self) {
        if self.state.status.id.is_some() {
            self.state.status.state = crate::project::SaveState::Saving;
            self.send(Command::SaveEditor(self.state.snapshot.editor.clone()));
        }
    }
    pub(super) fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.model_dialog_open {
            window.refresh();
        }
        self.settings.theme = crate::preferences::theme::current(cx);
        if let Some(error) = crate::preferences::theme::take_error(cx) {
            self.state.error = Some(error);
        }
        let events = self
            .service
            .as_ref()
            .map(|s| s.drain_events().collect::<Vec<_>>())
            .unwrap_or_default();
        for event in events {
            match self.state.apply(event) {
                Some(Effect::SyncInputs) => self.sync_fields(window, cx),
                Some(Effect::PlayAudio(bytes)) => match Playback::new(bytes) {
                    Ok(p) => self.playback = Some(p),
                    Err(e) => {
                        self.state.error = Some(format!("音声を再生できません: {e}"));
                        self.audio_id = None;
                    }
                },
                Some(Effect::Exported { directory, count }) => {
                    cx.spawn(async move |_, _| {
                        rfd::AsyncMessageDialog::new()
                            .set_title("エクスポート完了")
                            .set_description(format!(
                                "{count}件のWAVを書き出しました。\n{}",
                                directory.display()
                            ))
                            .show()
                            .await;
                    })
                    .detach();
                }
                Some(Effect::CloseWindow) => {
                    window.remove_window();
                    return;
                }
                None => {}
            }
        }
        if let Some(p) = &self.playback {
            if let Some(error) = p.take_error() {
                self.state.error = Some(error);
                self.audio_id = None;
            }
            let fraction = if p.duration > 0.0 {
                p.position() / p.duration
            } else {
                0.0
            };
            self.seek
                .update(cx, |s, cx| s.set_value(fraction, window, cx));
        }
        cx.notify();
    }
    pub(super) fn close(&mut self) {
        if let Some(error) = self.state.close_error() {
            self.state.error = Some(error.into());
            return;
        }
        self.autosave();
        self.send(Command::Flush { close: true });
    }
}
