//! Application settings and information in one dialog.
use super::Modal;
use crate::workspace::view::Studio;
use crate::workspace::view::appearance::{self, icon};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::*;

impl Studio {
    pub(in crate::workspace::view) fn show_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_modal(
            Modal::Settings,
            384.,
            || "設定".into_any_element(),
            |this, cx| {
                div()
                    .child(this.preferences_content(cx))
                    .child(div().h(px(1.)).mt_5().bg(this.palette().line))
                    .child(this.about_content())
                    .into_any_element()
            },
            window,
            cx,
        );
    }

    fn preferences_content(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .mt_5()
            .flex()
            .items_center()
            .justify_between()
            .gap_8()
            .child("カラーテーマ")
            .child(
                Button::new("preferences-theme")
                    .cursor_pointer()
                    .ghost()
                    .child(self.theme_track())
                    .on_click(cx.listener(|this, _, w, cx| this.toggle_theme(w, cx))),
            )
            .into_any_element()
    }

    fn about_content(&self) -> AnyElement {
        let p = self.palette();
        div()
            .mt_5()
            .flex()
            .flex_col()
            .gap_2()
            .text_sm()
            .child(div().text_color(p.muted).child("irodoruについて"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .child(icon("audio").size(px(24.)).text_color(p.accent))
                    .child("irodoru"),
            )
            .child(
                div()
                    .text_color(p.muted)
                    .child(format!("バージョン {}", env!("CARGO_PKG_VERSION"))),
            )
            .into_any_element()
    }

    fn theme_track(&self) -> AnyElement {
        let p = self.palette();
        div()
            .relative()
            .w(px(48.))
            .h(px(24.))
            .flex_shrink_0()
            .rounded_full()
            .border_1()
            .border_color(p.line)
            .bg(p.control)
            .flex()
            .items_center()
            .justify_between()
            .px_1()
            .child(
                div()
                    .absolute()
                    .top(px(2.))
                    .left(px(if self.settings.theme.is_light() {
                        2.
                    } else {
                        26.
                    }))
                    .size(px(18.))
                    .rounded_full()
                    .bg(p.ink),
            )
            .child(
                icon("sun")
                    .size(px(14.))
                    .text_color(if self.settings.theme.is_light() {
                        p.surface
                    } else {
                        p.muted
                    }),
            )
            .child(
                icon("moon")
                    .size(px(14.))
                    .text_color(if self.settings.theme.is_light() {
                        p.muted
                    } else {
                        p.surface
                    }),
            )
            .into_any_element()
    }

    fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.theme = crate::preferences::theme::toggle(cx);
        appearance::apply(self.settings.theme.is_light(), window, cx);
        if let Err(error) = crate::preferences::theme::save(self.settings.theme) {
            self.state.error = Some(format!("{error:#}"));
        }
        cx.notify();
    }
}
