use super::Studio;
use crate::workspace::protocol::Command;
use gpui_kit::*;

impl Studio {
    pub(super) fn choose_project(
        &mut self,
        create: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.busy || self.state.draft.is_some() {
            return;
        }
        self.state.busy = true;
        self.autosave();
        cx.spawn_in(window, async move |this, cx| {
            let dialog = rfd::AsyncFileDialog::new();
            let file = if create {
                dialog
                    .add_filter("irodoru プロジェクト", &["iro"])
                    .set_file_name("project.iro")
                    .save_file()
                    .await
            } else {
                dialog
                    .add_filter("irodoru プロジェクト", &["iro", "zip"])
                    .pick_file()
                    .await
            };
            let _ = this.update_in(cx, |this, _, cx| {
                if let Some(file) = file {
                    this.playback = None;
                    this.audio_id = None;
                    this.state.row_errors.clear();
                    this.send(Command::Open {
                        path: file.path().to_owned(),
                        create,
                    });
                } else {
                    this.state.busy = false;
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn import(&mut self, icon: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.busy {
            return;
        }
        self.state.busy = true;
        cx.spawn_in(window, async move |this, cx| {
            let file = rfd::AsyncFileDialog::new()
                .add_filter(
                    if icon { "画像" } else { "参照音声" },
                    if icon {
                        crate::audio::IMAGE_EXTENSIONS
                    } else {
                        crate::audio::AUDIO_EXTENSIONS
                    },
                )
                .pick_file()
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                if let Some(file) = file {
                    this.send(if icon {
                        Command::ImportIcon(file.path().to_owned())
                    } else {
                        Command::ImportVoice(file.path().to_owned())
                    });
                } else {
                    this.state.busy = false;
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn export(
        &mut self,
        id: String,
        preview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.busy {
            return;
        }
        self.state.busy = true;
        let filename = self
            .state
            .snapshot
            .takes
            .iter()
            .find(|t| t.id == id)
            .and_then(|t| t.filename.clone())
            .unwrap_or_else(|| "sample.wav".into());
        cx.spawn_in(window, async move |this, cx| {
            let file = rfd::AsyncFileDialog::new()
                .add_filter("WAV", &["wav"])
                .set_file_name(filename)
                .save_file()
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                if let Some(file) = file {
                    this.send(Command::Export {
                        id,
                        preview,
                        path: file.path().to_owned(),
                    });
                } else {
                    this.state.busy = false;
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn export_character(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.busy {
            return;
        }
        self.state.busy = true;
        cx.spawn_in(window, async move |this, cx| {
            let file = rfd::AsyncFileDialog::new()
                .add_filter("WAV", &["wav"])
                .set_file_name("sample.wav")
                .save_file()
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                if let Some(file) = file {
                    this.send(Command::ExportCharacter {
                        id,
                        path: file.path().to_owned(),
                    });
                } else {
                    this.state.busy = false;
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn cancel_character(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.busy {
            return;
        }
        let dirty = self.state.draft.as_ref().is_some_and(|d| {
            d.preview.is_some()
                || serde_json::to_value(&d.input).ok() != serde_json::to_value(&d.original).ok()
        });
        if !dirty {
            self.discard_character(window, cx);
            return;
        }
        self.state.busy = true;
        cx.spawn_in(window, async move |this, cx| {
            let choice = rfd::AsyncMessageDialog::new()
                .set_title("irodoru")
                .set_description("未保存の変更を破棄しますか？")
                .set_buttons(rfd::MessageButtons::YesNo)
                .show()
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.state.busy = false;
                if choice == rfd::MessageDialogResult::Yes {
                    this.discard_character(window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn delete_character(
        &mut self,
        id: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.busy {
            return;
        }
        self.state.busy = true;
        cx.spawn_in(window, async move |this, cx| {
            let choice = rfd::AsyncMessageDialog::new()
                .set_title("irodoru")
                .set_description(format!(
                    "「{name}」を削除しますか？生成済みの音声は残ります。"
                ))
                .set_buttons(rfd::MessageButtons::YesNo)
                .show()
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                if choice == rfd::MessageDialogResult::Yes {
                    this.state.saving_draft = this.state.draft.is_some();
                    this.send(Command::DeleteCharacter(id));
                } else {
                    this.state.busy = false;
                }
                cx.notify();
            });
        })
        .detach();
    }
}
