//! Model source settings, and the model status the footer shows on their trigger.
use super::Modal;
use crate::domain::InferenceStatus;
use crate::workspace::view::Studio;
use crate::workspace::view::appearance::DisabledWithCursor;
use crate::workspace::view::appearance::{Palette, button, icon, tool};
use gpui_kit::component::Sizable;
use gpui_kit::component::button::ButtonVariants;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tooltip::Tooltip;
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

pub(in crate::workspace::view) fn status_label(status: &InferenceStatus) -> String {
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

pub(in crate::workspace::view) fn status_appearance(
    status: &InferenceStatus,
    p: Palette,
) -> (&'static str, Hsla, bool) {
    match status {
        InferenceStatus::Ready => ("check", p.success, false),
        InferenceStatus::Generating => ("audio", p.accent, true),
        InferenceStatus::Error { .. } => ("alert", p.warning, false),
        InferenceStatus::Downloading { .. } => ("download", p.accent, false),
        InferenceStatus::Checking { .. }
        | InferenceStatus::Extracting { .. }
        | InferenceStatus::Loading => ("model", p.pending, true),
    }
}

impl Studio {
    pub(in crate::workspace::view) fn model_status(&self) -> InferenceStatus {
        self.inference
            .as_ref()
            .map(|service| service.status())
            .unwrap_or_else(|| InferenceStatus::Error {
                error: "モデル管理を開始できませんでした。アプリを再起動してください。".into(),
            })
    }

    pub(in crate::workspace::view) fn show_model_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.modal.is_some() {
            return;
        }
        self.model_error = None;
        self.model_details_open = false;
        self.open_modal(
            Modal::Model,
            420.,
            || {
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(icon("model"))
                    .child("モデル設定")
                    .into_any_element()
            },
            |this, cx| this.model_settings_content(cx),
            window,
            cx,
        );
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
        let model_source = self
            .inference
            .as_ref()
            .and_then(|service| service.model_source());
        let model_name = self
            .inference
            .as_ref()
            .and_then(|service| service.model_repository())
            .unwrap_or_else(|| {
                model_source
                    .clone()
                    .unwrap_or_else(|| "既定のモデル".into())
            });
        let error = self.model_error.clone().or_else(|| match &status {
            InferenceStatus::Error { error } => Some(error.clone()),
            _ => None,
        });
        let (icon_name, color, spinning) = status_appearance(&status, p);
        div()
            .mt_4()
            .flex()
            .flex_col()
            .gap_3()
            .text_sm()
            .child(
                div()
                    .p_3()
                    .rounded(px(8.))
                    .bg(p.control)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .id("model-name")
                            .w_full()
                            .overflow_hidden()
                            .text_ellipsis()
                            .font_weight(FontWeight::MEDIUM)
                            .child(model_name.clone())
                            .tooltip(move |window, cx| {
                                Tooltip::new(model_name.clone()).build(window, cx)
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .text_color(color)
                                    .child(if spinning {
                                        Spinner::new().small().color(color).into_any_element()
                                    } else {
                                        icon(icon_name).into_any_element()
                                    })
                                    .child(status_label(&status)),
                            )
                            .when(failed, |el| {
                                el.child(
                                    tool("retry-models", "refresh", "再試行")
                                        .accessibility_label("再試行")
                                        .disabled_with_cursor(!editable)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.model_error = this
                                                .inference
                                                .as_ref()
                                                .and_then(|service| service.retry().err())
                                                .map(|error| format!("{error:#}"));
                                            window.refresh();
                                            cx.notify();
                                        })),
                                )
                            }),
                    )
                    .when_some(progress(&status), |el, (done, total)| {
                        el.when(total > 0, |el| {
                            el.child(
                                div()
                                    .h(px(4.))
                                    .rounded_full()
                                    .bg(p.line)
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative((done as f32 / total as f32).min(1.)))
                                            .bg(p.accent),
                                    ),
                            )
                        })
                    }),
            )
            .when(self.model_error.is_some(), |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .text_color(p.warning)
                        .child(icon("alert"))
                        .child("処理に失敗しました。詳細をご確認ください。"),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        button("model-details", "詳細")
                            .ghost()
                            .text_color(p.muted)
                            .icon(icon(if self.model_details_open {
                                "chevron-up"
                            } else {
                                "chevron-down"
                            }))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.model_details_open = !this.model_details_open;
                                window.refresh();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                button("import-model-archive", "モデル変更")
                                    .icon(icon("folder"))
                                    .tooltip("モデルZIPを取り込む")
                                    .disabled_with_cursor(!editable)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.choose_model_archive(window, cx)
                                    })),
                            )
                            .child(
                                tool("reset-model-archive", "reset", "既定のモデルに戻す")
                                    .accessibility_label("既定のモデルに戻す")
                                    .disabled_with_cursor(!editable || model_source.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.configure_model_archive(None, cx)
                                    })),
                            ),
                    ),
            )
            .when(self.model_details_open, |el| {
                el.child(self.model_details(&status, error))
            })
            .when(!editable && self.inference.is_some(), |el| {
                el.child(
                    div()
                        .text_xs()
                        .text_color(p.muted)
                        .child(if self.model_picker_open {
                            "ZIPファイルを選択中…"
                        } else {
                            "処理が完了するとモデルを変更できます。"
                        }),
                )
            })
            .into_any_element()
    }

    fn model_details(&self, status: &InferenceStatus, error: Option<String>) -> AnyElement {
        let p = self.palette();
        let providers = self
            .inference
            .as_ref()
            .map(|service| service.providers())
            .unwrap_or_default();
        div().id("model-details-content").max_h(px(224.)).overflow_y_scroll()
            .border_t_1().border_color(p.line).pt_3()
            .flex().flex_col().gap_3().text_xs().text_color(p.muted)
            .when_some(error, |el, error| el.child(
                div().p_3().rounded(px(8.)).bg(p.warning_soft).text_color(p.warning).child(error),
            ))
            .when(!providers.is_empty(), |el| el.child(
                div().flex().flex_col().gap_1()
                    .children(providers.into_iter().map(|provider| {
                        div().child(format!("{}：{}{}", provider.graph, provider.provider,
                            if provider.fallback { "（CPUフォールバック）" } else { "" }))
                    }))
                    .child("一部の演算はCPUで実行される場合があります。"),
            ))
            .when_some(progress(status), |el, (done, total)| {
                el.child(div().child(if total > 0 {
                    format!("{:.1} / {:.1} MiB", done as f64 / 1_048_576.0, total as f64 / 1_048_576.0)
                } else {
                    format!("{:.1} MiB", done as f64 / 1_048_576.0)
                }))
            })
            .child("変更はすべてのウィンドウと次回起動時に反映されます。取り込み後のZIPは削除できます。")
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
