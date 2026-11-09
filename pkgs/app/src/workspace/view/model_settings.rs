//! Shared model status in the footer and model source settings in a modal.
use super::appearance::{button, icon, tool};
use super::{Studio, appearance};
use crate::domain::InferenceStatus;
use gpui_kit::component::button::ButtonVariants;
use gpui_kit::component::input::Input;
use gpui_kit::component::{Disableable, WindowExt};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

fn progress(status: &InferenceStatus) -> Option<(u64, u64)> {
    match *status {
        InferenceStatus::Checking { checked, total } => Some((checked, total)),
        InferenceStatus::Downloading { downloaded, total } => Some((downloaded, total)),
        InferenceStatus::Extracting { extracted, total } => Some((extracted, total)),
        _ => None,
    }
}

fn status_label(status: &InferenceStatus) -> String {
    let label = match status {
        InferenceStatus::Checking { .. } => "モデル確認中",
        InferenceStatus::Downloading { .. } => "ダウンロード中",
        InferenceStatus::Extracting { .. } => "展開中",
        InferenceStatus::Loading => "読み込み中",
        InferenceStatus::Ready => "準備完了",
        InferenceStatus::Generating => "音声生成中",
        InferenceStatus::Error { .. } => "モデルエラー",
    };
    match progress(status) {
        Some((done, total)) if total > 0 => {
            let percent = (done as f64 / total as f64 * 100.0).min(100.0);
            format!("{label} {percent:.0}%")
        }
        _ => label.into(),
    }
}

impl Studio {
    fn model_status(&self) -> InferenceStatus {
        self.service
            .as_ref()
            .map(|service| service.status())
            .unwrap_or_else(|| InferenceStatus::Error {
                error: "モデル管理を開始できませんでした。アプリを再起動してください。".into(),
            })
    }

    pub(super) fn model_status_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let status = self.model_status();
        let failed = matches!(status, InferenceStatus::Error { .. });
        button("model-settings", "")
            .ghost()
            .track_focus(&self.model_trigger_focus)
            .flex_shrink_0()
            .h(px(24.))
            .icon(icon("model"))
            .label(status_label(&status))
            .text_color(if failed { p.warning } else { p.muted })
            .tooltip("モデル設定")
            .on_click(cx.listener(|this, _, window, cx| this.show_model_settings(window, cx)))
            .into_any_element()
    }

    fn show_model_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog_open {
            return;
        }
        let directory = self
            .service
            .as_ref()
            .and_then(|service| service.model_directory())
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        self.fields
            .model_directory
            .update(cx, |input, cx| input.set_value(directory, window, cx));
        self.dialog_open = true;
        self.model_dialog_open = true;
        self.model_error = None;
        let weak = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let content = weak
                .update(cx, |this, cx| this.model_settings_content(cx))
                .unwrap_or_else(|_| div().into_any_element());
            let p = appearance::Palette::new(crate::preferences::theme::light(cx));
            let on_close = weak.clone();
            dialog
                .title(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(icon("model"))
                                .child("モデル設定"),
                        )
                        .child(tool("model-dialog-close", "close", "閉じる").on_click(
                            |_, window, cx| {
                                window.close_dialog(cx);
                            },
                        )),
                )
                .centered()
                .close_button(false)
                .width(px(480.))
                .p_5()
                .rounded(px(12.))
                .bg(p.panel)
                .text_color(p.ink)
                .border_color(p.line)
                .overlay_closable(true)
                .child(content)
                .on_close(move |_, window, cx| {
                    let _ = on_close.update(cx, |this, cx| {
                        this.dialog_open = false;
                        this.model_dialog_open = false;
                        this.model_trigger_focus.focus(window, cx);
                        cx.notify();
                    });
                })
        });
    }

    fn model_settings_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let status = self.model_status();
        let failed = matches!(status, InferenceStatus::Error { .. });
        let editable = self.service.is_some()
            && !self.state.busy
            && !self.model_picker_open
            && matches!(
                status,
                InferenceStatus::Ready | InferenceStatus::Error { .. }
            );
        let directory = self
            .service
            .as_ref()
            .and_then(|service| service.model_directory());
        let entered_path = self
            .fields
            .model_directory
            .read(cx)
            .value()
            .trim()
            .to_owned();
        let can_apply = editable && !entered_path.is_empty();
        let source = directory
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "既定のモデル".into());
        let error = self.model_error.clone().or_else(|| match &status {
            InferenceStatus::Error { error } => Some(error.clone()),
            _ => None,
        });
        div()
            .mt_4().flex().flex_col().gap_3().text_sm()
            .child(div().flex().items_center().justify_between()
                .child("モデル：anime")
                .child(div().text_color(if failed { p.warning } else { p.muted })
                    .child(status_label(&status))))
            .when_some(progress(&status), |el, (done, total)| {
                el.child(div().text_xs().text_color(p.muted).child(if total > 0 {
                    format!("{:.1} / {:.1} MiB", done as f64 / 1_048_576.0, total as f64 / 1_048_576.0)
                } else {
                    format!("{:.1} MiB", done as f64 / 1_048_576.0)
                }))
                .when(total > 0, |el| el.child(
                    div().h(px(4.)).rounded_full().bg(p.control).overflow_hidden()
                        .child(div().h_full().w(relative((done as f32 / total as f32).min(1.)))
                            .bg(p.accent)),
                ))
            })
            .when_some(error, |el, error| el.child(
                div().id("model-error-details").max_h(px(128.)).overflow_y_scroll()
                    .p_3().rounded(px(8.)).bg(p.warning_soft).text_color(p.warning).child(error),
            ))
            .child(div().h(px(1.)).bg(p.line))
            .child(div().text_color(p.muted).child("モデルフォルダ"))
            .child(div().id("model-directory").max_h(px(72.)).overflow_y_scroll().child(source))
            .child(Input::new(&self.fields.model_directory).disabled(!editable))
            .child(div().text_xs().text_color(p.muted)
                .child("anime と codec を含むフォルダを選択するか、パスを入力して適用してください。設定はすべてのウィンドウと次回起動時に使用します。"))
            .child(div().flex().flex_wrap().gap_2()
                .child(button("choose-model-directory", "フォルダを選択…")
                    .icon(icon("folder")).disabled(!editable)
                    .when(!editable, |el| el.cursor_default())
                    .on_click(cx.listener(|this, _, window, cx| this.choose_model_directory(window, cx))))
                .child(button("apply-model-directory", "適用")
                    .disabled(!can_apply)
                    .when(!can_apply, |el| el.cursor_default())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.configure_model_directory(Some(std::path::PathBuf::from(&entered_path)), cx);
                    })))
                .child(button("reset-model-directory", "既定に戻す")
                    .disabled(!editable || directory.is_none())
                    .when(!editable || directory.is_none(), |el| el.cursor_default())
                    .on_click(cx.listener(|this, _, _, cx| this.configure_model_directory(None, cx))))
                .when(failed, |el| el.child(
                    button("retry-models", "再試行")
                        .disabled(!editable)
                        .when(!editable, |el| el.cursor_default())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.model_error = this.service.as_ref()
                                .and_then(|service| service.retry_models().err())
                                .map(|error| format!("{error:#}"));
                            cx.notify();
                        })),
                )))
            .when(!editable && self.service.is_some(), |el| el.child(
                div().text_xs().text_color(p.muted).child(if self.model_picker_open {
                    "フォルダを選択中…"
                } else {
                    "処理が完了するとモデルフォルダを変更できます。"
                }),
            ))
            .into_any_element()
    }

    fn choose_model_directory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.model_picker_open || self.state.busy {
            return;
        }
        self.model_picker_open = true;
        self.model_error = None;
        let current = self
            .service
            .as_ref()
            .and_then(|service| service.model_directory());
        cx.spawn_in(window, async move |this, cx| {
            let mut picker = rfd::AsyncFileDialog::new().set_title("モデルフォルダを選択");
            if let Some(path) = current {
                picker = picker.set_directory(path);
            }
            let selected = picker.pick_folder().await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.model_picker_open = false;
                if let Some(folder) = selected {
                    this.fields.model_directory.update(cx, |input, cx| {
                        input.set_value(folder.path().display().to_string(), window, cx);
                    });
                    this.configure_model_directory(Some(folder.path().to_owned()), cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn configure_model_directory(
        &mut self,
        directory: Option<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) {
        self.model_error = self
            .service
            .as_ref()
            .and_then(|service| service.configure_models(directory).err())
            .map(|error| format!("{error:#}"));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::status_label;
    use crate::domain::InferenceStatus;

    #[test]
    fn status_labels_handle_unknown_and_complete_downloads() {
        assert_eq!(
            status_label(&InferenceStatus::Downloading {
                downloaded: 5,
                total: 0
            }),
            "ダウンロード中"
        );
        assert_eq!(
            status_label(&InferenceStatus::Downloading {
                downloaded: 1,
                total: 4
            }),
            "ダウンロード中 25%"
        );
        assert_eq!(
            status_label(&InferenceStatus::Downloading {
                downloaded: 5,
                total: 4
            }),
            "ダウンロード中 100%"
        );
        assert_eq!(status_label(&InferenceStatus::Ready), "準備完了");
    }
}
