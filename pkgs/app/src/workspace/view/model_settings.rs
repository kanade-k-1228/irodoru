//! Shared model status in the footer and model source settings in a modal.
use super::appearance::{button, icon, tool};
use super::{Studio, appearance};
use crate::domain::InferenceStatus;
use gpui_kit::component::button::ButtonVariants;
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
        self.inference
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
        let editable = self.inference.is_some()
            && !self.state.busy
            && !self.model_picker_open
            && matches!(
                status,
                InferenceStatus::Ready | InferenceStatus::Error { .. }
            );
        let model_name = self
            .inference
            .as_ref()
            .and_then(|service| service.model_name());
        let source = model_name.clone().unwrap_or_else(|| "既定のモデル".into());
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
            .child(div().text_color(p.muted).child("使用するモデル"))
            .child(div().id("model-source").max_h(px(72.)).overflow_y_scroll().child(source))
            .child(div().text_xs().text_color(p.muted)
                .child("モデルZIPを取り込むと、すべてのウィンドウと次回起動時に使用します。取り込み後は元のZIPを移動・削除しても使えます。"))
            .child(div().flex().flex_wrap().gap_2()
                .child(button("import-model-archive", "ZIPファイルを選択して取り込む…")
                    .icon(icon("folder")).disabled(!editable)
                    .when(!editable, |el| el.cursor_default())
                    .on_click(cx.listener(|this, _, window, cx| this.choose_model_archive(window, cx))))
                .child(button("reset-model-archive", "既定に戻す")
                    .disabled(!editable || model_name.is_none())
                    .when(!editable || model_name.is_none(), |el| el.cursor_default())
                    .on_click(cx.listener(|this, _, _, cx| this.configure_model_archive(None, cx))))
                .when(failed, |el| el.child(
                    button("retry-models", "再試行")
                        .disabled(!editable)
                        .when(!editable, |el| el.cursor_default())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.model_error = this.inference.as_ref()
                                .and_then(|service| service.retry().err())
                                .map(|error| format!("{error:#}"));
                            cx.notify();
                        })),
                )))
            .when(!editable && self.inference.is_some(), |el| el.child(
                div().text_xs().text_color(p.muted).child(if self.model_picker_open {
                    "ZIPファイルを選択中…"
                } else {
                    "処理が完了するとモデルを変更できます。"
                }),
            ))
            .into_any_element()
    }

    fn choose_model_archive(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.model_picker_open || self.state.busy {
            return;
        }
        self.model_picker_open = true;
        self.model_error = None;
        cx.spawn_in(window, async move |this, cx| {
            let selected = rfd::AsyncFileDialog::new()
                .set_title("モデルZIPを取り込む")
                .add_filter("モデルZIP", &["zip"])
                .pick_file()
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                this.model_picker_open = false;
                if let Some(file) = selected {
                    this.configure_model_archive(Some(file.path().to_owned()), cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn configure_model_archive(
        &mut self,
        archive: Option<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) {
        self.model_error = self
            .inference
            .as_ref()
            .and_then(|service| service.configure_models(archive).err())
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
