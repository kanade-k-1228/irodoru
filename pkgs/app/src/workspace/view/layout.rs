use super::Studio;
use super::appearance::{button, icon, tool};
use crate::domain::InferenceStatus;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Render for Studio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette();
        let _ = &self.subscriptions;
        div()
            .relative()
            .track_focus(&self.focus)
            .on_action(
                cx.listener(|this, _: &super::actions::NewWindow, w, cx| {
                    this.menu_select(0, w, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &super::actions::NewProject, w, cx| {
                    this.menu_select(1, w, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &super::actions::OpenProject, w, cx| {
                this.menu_select(2, w, cx)
            }))
            .on_action(cx.listener(|this, _: &super::actions::SaveProject, w, cx| {
                this.menu_select(3, w, cx)
            }))
            .on_action(cx.listener(|this, _: &super::actions::Preferences, w, cx| {
                this.menu_select(5, w, cx)
            }))
            .on_action(cx.listener(|this, _: &super::actions::CloseWindow, w, cx| {
                this.menu_select(7, w, cx)
            }))
            .on_action(
                cx.listener(|this, _: &super::actions::Quit, w, cx| this.menu_select(7, w, cx)),
            )
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(p.surface)
            .text_color(p.ink)
            .text_sm()
            .child(self.header(window, cx))
            .when_some(self.model_status_row(cx), |el, status| el.child(status))
            .when_some(self.state.error.clone(), |el, error| {
                el.child(self.error_banner(error, cx))
            })
            .child(self.script(cx))
            .child(self.editor_panel(window, cx))
            .child(self.project_path())
            .when(self.menu_open, |el| el.child(self.app_menu(cx)))
    }
}

impl Studio {
    fn model_status_row(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (message, failed) = match self.service.as_ref()?.status() {
            InferenceStatus::Checking { checked, total } => {
                (model_progress("確認中", checked, total), false)
            }
            InferenceStatus::Downloading { downloaded, total } => {
                (model_progress("ダウンロード中", downloaded, total), false)
            }
            InferenceStatus::Extracting { extracted, total } => {
                (model_progress("展開中", extracted, total), false)
            }
            InferenceStatus::Loading => ("anime · モデルを読み込み中".into(), false),
            InferenceStatus::Error { error } => (format!("モデルエラー: {error}"), true),
            InferenceStatus::Ready | InferenceStatus::Generating => return None,
        };
        let p = self.palette();
        Some(
            div()
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap_2()
                .px_3()
                .py_1()
                .border_b_1()
                .border_color(p.line)
                .bg(p.panel)
                .text_xs()
                .text_color(p.muted)
                .when(failed, |el| el.bg(p.warning_soft).text_color(p.warning))
                .child(div().flex_1().min_w_0().child(message))
                .when(failed, |el| {
                    el.child(button("retry-models", "再試行").on_click(cx.listener(
                        |this, _, _, cx| {
                            if let Some(service) = &this.service
                                && let Err(error) = service.retry_models()
                            {
                                this.state.error = Some(error.to_string());
                            }
                            cx.notify();
                        },
                    )))
                })
                .into_any_element(),
        )
    }

    fn error_banner(&self, error: String, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .bg(p.warning_soft)
            .text_color(p.warning)
            .child(icon("alert"))
            .child(div().flex_1().child(error))
            .child(
                tool("dismiss-error", "close", "エラーを閉じる").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.state.error = None;
                        cx.notify();
                    },
                )),
            )
            .into_any_element()
    }

    fn editor_panel(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let narrow = window.viewport_size().width < px(640.);
        div()
            .flex()
            .h(px(
                (f32::from(window.viewport_size().height) * 0.5).min(360.)
            ))
            .flex_shrink_0()
            .min_h_0()
            .border_t_1()
            .border_color(p.line)
            .bg(p.panel)
            .child(self.characters(narrow, cx))
            .child(if self.state.draft.is_some() {
                self.character_editor(narrow, cx)
            } else {
                self.line_editor(narrow, cx)
            })
            .into_any_element()
    }

    fn project_path(&self) -> AnyElement {
        let p = self.palette();
        div()
            .flex_shrink_0()
            .border_t_1()
            .border_color(p.line)
            .bg(p.panel)
            .px_2()
            .py_1()
            .text_xs()
            .line_height(px(16.))
            .text_color(p.muted)
            .overflow_hidden()
            .whitespace_nowrap()
            .child(
                self.state
                    .status
                    .path
                    .clone()
                    .unwrap_or_else(|| "プロジェクト未選択".into()),
            )
            .into_any_element()
    }
}

fn model_progress(label: &str, completed: u64, total: u64) -> String {
    let percent = if total == 0 {
        0.0
    } else {
        (completed as f64 / total as f64 * 100.0).min(100.0)
    };
    format!(
        "anime · {label} {percent:.0}% ({:.1} / {:.1} MiB)",
        completed as f64 / 1_048_576.0,
        total as f64 / 1_048_576.0,
    )
}
