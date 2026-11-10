use super::Studio;

pub(super) enum ProjectDialog {
    New,
    Open,
    FromCharas,
}
use crate::domain::AudioSource;
use crate::workspace::protocol::Command;
use gpui_kit::*;

impl Studio {
    /// Shows a native dialog while the workspace is busy, then sends the command chosen
    /// for its answer. Cancelling, or answering with no command, ends the busy state.
    fn ask<T: 'static>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        dialog: impl Future<Output = Option<T>> + 'static,
        answer: impl FnOnce(&mut Self, T) -> Option<Command> + 'static,
    ) {
        self.state.busy = true;
        cx.spawn_in(window, async move |this, cx| {
            let picked = dialog.await;
            let _ = this.update_in(cx, |this, _, cx| {
                match picked.and_then(|value| answer(this, value)) {
                    Some(command) => this.send(command),
                    None => this.state.busy = false,
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn choose_project(
        &mut self,
        action: ProjectDialog,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.busy {
            return;
        }
        let create = !matches!(action, ProjectDialog::Open);
        let dialog = async move {
            let dialog =
                rfd::AsyncFileDialog::new().add_filter("irodoru プロジェクト", &["iro", "IRO"]);
            if create {
                dialog.set_file_name("project.iro").save_file().await
            } else {
                dialog.pick_file().await
            }
        };
        self.ask(window, cx, dialog, move |this, file| {
            let path = file.path().to_owned();
            if let Err(error) = crate::project::validate_path(&path) {
                this.state.error = Some(error.to_string());
                return None;
            }
            this.autosave();
            this.stop_audio();
            this.state.line_errors.clear();
            Some(if matches!(action, ProjectDialog::FromCharas) {
                Command::NewFromCharas(path)
            } else {
                Command::Open { path, create }
            })
        });
    }
    pub(super) fn import(&mut self, icon: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.busy || self.state.chara_locked {
            return;
        }
        let Some(chara_id) = self.state.active_chara().map(|chara| chara.id.clone()) else {
            return;
        };
        let dialog = async move {
            let dialog = rfd::AsyncFileDialog::new().add_filter(
                if icon { "画像" } else { "参照音声" },
                if icon {
                    crate::icons::IMAGE_EXTENSIONS
                } else {
                    crate::audio::AUDIO_EXTENSIONS
                },
            );
            if icon {
                dialog.pick_file().await.map(|f| vec![f])
            } else {
                dialog.pick_files().await
            }
        };
        self.ask(window, cx, dialog, move |_, files| {
            let mut paths = files.into_iter().map(|f| f.path().to_owned());
            if icon {
                let path = paths.next()?;
                Some(Command::ImportIcon { chara_id, path })
            } else {
                Some(Command::ImportVoices {
                    chara_id,
                    paths: paths.collect(),
                })
            }
        });
    }
    pub(super) fn chara_file(&mut self, export: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.busy || self.state.status.id.is_none() {
            return;
        }
        let input = if export {
            let Some(chara) = self.state.active_chara() else {
                return;
            };
            Some(chara.clone().into())
        } else {
            None
        };
        let dialog = async move {
            let dialog =
                rfd::AsyncFileDialog::new().add_filter("irodoru キャラクター", &["irochar"]);
            if export {
                dialog.set_file_name("chara.irochar").save_file().await
            } else {
                dialog.pick_file().await
            }
        };
        self.ask(window, cx, dialog, move |_, file| {
            let path = file.path().to_owned();
            Some(match input {
                Some(input) => Command::ExportChara { input, path },
                None => Command::ImportChara(path),
            })
        });
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
        let filename = match &source {
            AudioSource::Take(id) => self
                .state
                .snapshot
                .takes
                .iter()
                .find(|take| &take.id == id)
                .and_then(|take| take.conditions.filename().ok()),
            AudioSource::Reference(id) => self
                .state
                .snapshot
                .voices
                .iter()
                .find(|entry| entry.voice.id() == id)
                .and_then(|entry| match &entry.voice {
                    crate::domain::Voice::Imported { name, .. } => {
                        let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
                        std::path::Path::new(name)
                            .file_stem()
                            .and_then(|stem| stem.to_str())
                            .map(|stem| format!("{stem}.wav"))
                    }
                    crate::domain::Voice::Generated(take) => take.conditions.filename().ok(),
                }),
        }
        .unwrap_or_else(|| "audio.wav".into());
        let dialog = async move {
            rfd::AsyncFileDialog::new()
                .add_filter("WAV", &["wav"])
                .set_file_name(filename)
                .save_file()
                .await
        };
        self.ask(window, cx, dialog, move |_, file| {
            Some(Command::Export {
                source,
                path: file.path().to_owned(),
            })
        });
    }
    pub(super) fn export_lines(
        &mut self,
        naming: crate::project::ExportNaming,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.busy || self.state.status.id.is_none() {
            return;
        }
        if let Err(error) =
            crate::project::export_lines(&self.state.snapshot.editor, &self.state.snapshot.takes)
        {
            self.state.error = Some(error.to_string());
            cx.notify();
            return;
        }
        self.autosave();
        let dialog = async move {
            rfd::AsyncFileDialog::new()
                .set_title("セリフごとのWAVの書き出し先")
                .pick_folder()
                .await
        };
        self.ask(window, cx, dialog, move |_, folder| {
            Some(Command::ExportLines {
                parent: folder.path().to_owned(),
                naming,
            })
        });
        cx.notify();
    }

    pub(super) fn delete_chara(
        &mut self,
        id: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.busy || self.state.chara_locked || self.state.chara_in_use(&id) {
            return;
        }
        let dialog = async move {
            let choice = rfd::AsyncMessageDialog::new()
                .set_title("irodoru")
                .set_description(format!(
                    "「{name}」を削除しますか？生成済みの音声は残ります。"
                ))
                .set_buttons(rfd::MessageButtons::YesNo)
                .show()
                .await;
            (choice == rfd::MessageDialogResult::Yes).then_some(())
        };
        self.ask(window, cx, dialog, move |_, ()| {
            Some(Command::DeleteChara(id))
        });
    }
}
