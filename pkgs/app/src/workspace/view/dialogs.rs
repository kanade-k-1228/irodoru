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
                    this.state.line_errors.clear();
                    this.send(if matches!(action, ProjectDialog::FromCharas) {
                        Command::NewFromCharas(file.path().to_owned())
                    } else {
                        Command::Open {
                            path: file.path().to_owned(),
                            create,
                        }
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
        if self.state.busy || self.state.chara_locked {
            return;
        }
        let Some(chara_id) = self.state.active_chara().map(|chara| chara.id.clone()) else {
            return;
        };
        self.state.busy = true;
        cx.spawn_in(window, async move |this, cx| {
            let dialog = rfd::AsyncFileDialog::new().add_filter(
                if icon { "画像" } else { "参照音声" },
                if icon {
                    crate::icons::IMAGE_EXTENSIONS
                } else {
                    crate::audio::AUDIO_EXTENSIONS
                },
            );
            let files = if icon {
                dialog.pick_file().await.map(|f| vec![f])
            } else {
                dialog.pick_files().await
            };
            let _ = this.update_in(cx, |this, _, cx| {
                if let Some(files) = files {
                    let paths: Vec<_> = files.into_iter().map(|f| f.path().to_owned()).collect();
                    if icon {
                        if let Some(path) = paths.into_iter().next() {
                            this.send(Command::ImportIcon { chara_id, path });
                        } else {
                            this.state.busy = false;
                        }
                    } else {
                        this.send(Command::ImportVoices { chara_id, paths });
                    }
                } else {
                    this.state.busy = false;
                }
                cx.notify();
            });
        })
        .detach();
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
        self.state.busy = true;
        cx.spawn_in(window, async move |this, cx| {
            let dialog =
                rfd::AsyncFileDialog::new().add_filter("irodoru キャラクター", &["irochar"]);
            let file = if export {
                dialog.set_file_name("chara.irochar").save_file().await
            } else {
                dialog.pick_file().await
            };
            let _ = this.update_in(cx, |this, _, cx| {
                if let Some(file) = file {
                    this.send(match input {
                        Some(input) => Command::ExportChara {
                            input,
                            path: file.path().to_owned(),
                        },
                        None => Command::ImportChara(file.path().to_owned()),
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
            .and_then(|t| t.conditions.filename().ok())
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
        self.state.busy = true;
        cx.spawn_in(window, async move |this, cx| {
            let folder = rfd::AsyncFileDialog::new()
                .set_title("セリフごとのWAVの書き出し先")
                .pick_folder()
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                if let Some(folder) = folder {
                    this.send(Command::ExportLines {
                        parent: folder.path().to_owned(),
                        naming,
                    });
                } else {
                    this.state.busy = false;
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn delete_chara(
        &mut self,
        id: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.busy || self.state.chara_locked {
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
                    let command = this.state.delete_chara(id);
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
