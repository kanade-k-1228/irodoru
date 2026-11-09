//! Application settings and information in one dialog.
use super::appearance::{icon, tool};
use super::{Studio, appearance};
use gpui_kit::component::WindowExt;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::*;

impl Studio {
    pub(super) fn show_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog_open {
            return;
        }
        self.dialog_open = true;
        let weak = cx.entity().downgrade();
        let on_close = weak.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let content = weak
                .update(cx, |this, cx| {
                    div()
                        .child(this.preferences_content(cx))
                        .child(div().h(px(1.)).mt_5().bg(this.palette().line))
                        .child(this.about_content())
                        .into_any_element()
                })
                .unwrap_or_else(|_| div().into_any_element());
            let p = appearance::Palette::new(crate::preferences::theme::light(cx));
            let on_close = on_close.clone();
            dialog
                .title(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child("設定")
                        .child(tool("dialog-close", "close", "閉じる").on_click(
                            |_, window, cx| {
                                window.close_dialog(cx);
                            },
                        )),
                )
                .centered()
                .close_button(false)
                .width(px(384.))
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
                        this.menu_trigger_focus.focus(window, cx);
                        cx.notify();
                    });
                })
        });
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
            .child("キャラクタの声で台本を読み上げる音声合成アプリ")
            .into_any_element()
    }
}
