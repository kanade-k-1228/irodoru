//! The window's modal dialogs: the shared frame here, one module per dialog.
use super::appearance::tool;
use super::{Studio, appearance};
use gpui_kit::component::WindowExt;
use gpui_kit::*;

mod model;
mod settings;
pub(super) use model::status_label;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Modal {
    Settings,
    Model,
}

impl Studio {
    /// Opens the one modal the window may show and returns focus to its trigger on close.
    fn open_modal(
        &mut self,
        kind: Modal,
        width: f32,
        title: impl Fn() -> AnyElement + 'static,
        content: impl Fn(&mut Self, &mut Context<Self>) -> AnyElement + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.modal.is_some() {
            return;
        }
        self.modal = Some(kind);
        let weak = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let body = weak
                .update(cx, |this, cx| content(this, cx))
                .unwrap_or_else(|_| div().into_any_element());
            let p = appearance::Palette::new(crate::preferences::theme::light(cx));
            let on_close = weak.clone();
            dialog
                .title(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(title())
                        .child(tool("dialog-close", "close", "閉じる").on_click(
                            |_, window, cx| {
                                window.close_dialog(cx);
                            },
                        )),
                )
                .centered()
                .close_button(false)
                .width(px(width))
                .p_5()
                .rounded(px(12.))
                .bg(p.panel)
                .text_color(p.ink)
                .border_color(p.line)
                .overlay_closable(true)
                .child(body)
                .on_close(move |_, window, cx| {
                    let _ = on_close.update(cx, |this, cx| {
                        let trigger = match this.modal.take() {
                            Some(Modal::Model) => this.model_trigger_focus.clone(),
                            _ => this.menu_trigger_focus.clone(),
                        };
                        trigger.focus(window, cx);
                        cx.notify();
                    });
                })
        });
    }
}
