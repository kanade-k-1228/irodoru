use super::Studio;
use crate::domain::AudioSource;
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
        cx.spawn_in(window, async move |this, cx| {
            let dialog =
                rfd::AsyncFileDialog::new().add_filter("irodoru プロジェクト", &["iro", "IRO"]);
            let file = if create {
                dialog.set_file_name("project.iro").save_file().await
            } else {
                dialog.pick_file().await
            };
            let _ = this.update_in(cx, |this, _, cx| {
                if let Some(file) = file {
                    if let Err(error) = crate::project::validate_path(file.path()) {
                        this.state.busy = false;
                        this.state.error = Some(error.to_string());
                        cx.notify();
                        return;
                    }
                    this.autosave();
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
                        crate::icons::IMAGE_EXTENSIONS
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
        source: AudioSource,
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
            .find(|t| matches!(&source, AudioSource::Take(id) if &t.id == id))
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
                        source,
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
        let dirty = self.state.draft.as_ref().is_some_and(|d| d.is_dirty());
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
                    let command = this.state.delete_character(id);
                    this.send(command);
                } else {
                    this.state.busy = false;
                }
                cx.notify();
            });
        })
        .detach();
    }
}
