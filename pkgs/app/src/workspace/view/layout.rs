use super::Studio;
use super::app_menu::MenuAction;
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
            .on_action(cx.listener(|this, _: &super::actions::NewWindow, w, cx| {
                this.menu_select(MenuAction::NewWindow, w, cx)
            }))
            .on_action(cx.listener(|this, _: &super::actions::NewProject, w, cx| {
                this.menu_select(MenuAction::NewProject, w, cx)
            }))
            .on_action(cx.listener(|this, _: &super::actions::OpenProject, w, cx| {
                this.menu_select(MenuAction::OpenProject, w, cx)
            }))
            .on_action(cx.listener(|this, _: &super::actions::SaveProject, w, cx| {
                this.menu_select(MenuAction::SaveProject, w, cx)
            }))
            .on_action(cx.listener(|this, _: &super::actions::Preferences, w, cx| {
                this.menu_select(MenuAction::Preferences, w, cx)
            }))
            .on_action(cx.listener(|this, _: &super::actions::CloseWindow, w, cx| {
                this.menu_select(MenuAction::CloseWindow, w, cx)
            }))
            .on_action(cx.listener(|this, _: &super::actions::Quit, w, cx| {
                this.menu_select(MenuAction::CloseWindow, w, cx)
            }))
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(p.surface)
            .text_color(p.ink)
            .text_sm()
            .when(!cfg!(target_os = "macos"), |el| {
                el.child(self.header(window, cx))
            })
            .when_some(self.state.error.clone(), |el, error| {
                el.child(self.error_banner(error, cx))
            })
            .child(self.script(cx))
            .child(self.character_panel(window, cx))
            .child(self.footer(cx))
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
}
