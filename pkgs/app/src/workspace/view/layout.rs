use super::Studio;
use super::appearance::{icon, tool};
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
