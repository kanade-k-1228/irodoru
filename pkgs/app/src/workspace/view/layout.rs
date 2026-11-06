use super::Studio;
use super::appearance::{icon, tool};
use crate::domain::InferenceStatus;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Render for Studio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let narrow = window.viewport_size().width < px(640.);
        let p = self.palette();
        let inference = self
            .service
            .as_ref()
            .map(|s| match s.status() {
                InferenceStatus::Loading => "anime · モデルを読み込み中".to_owned(),
                InferenceStatus::Ready => "anime · 生成できます".into(),
                InferenceStatus::Generating => "anime · 生成中".into(),
                InferenceStatus::Error { error } => format!("モデルエラー: {error}"),
            })
            .unwrap_or_default();
        let status = match self.state.status.state.as_str() {
            "saved" => "保存済み",
            "saving" => "保存中…",
            "error" => "保存失敗",
            _ => "",
        };
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
            .child(self.header(inference, status, window, cx))
            .when_some(self.state.error.clone(), |el, error| {
                el.child(
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
                        .child(tool("dismiss-error", "close", "エラーを閉じる").on_click(
                            cx.listener(|this, _, _, cx| {
                                this.state.error = None;
                                cx.notify();
                            }),
                        )),
                )
            })
            .child(self.script(cx))
            .child(
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
                    }),
            )
            .child(
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
                    ),
            )
            .when(self.menu_open, |el| el.child(self.app_menu(cx)))
    }
}
