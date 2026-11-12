//! The window's modals: the shared dialog frame here, one module per modal.
use super::{Studio, appearance};
use gpui_kit::component::WindowExt;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::*;

mod export;
mod menu;
mod model;
mod settings;
pub(super) use menu::MenuAction;
pub(super) use model::{status_appearance, status_label};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Modal {
    Menu,
    Settings,
    Model,
    Export,
}

impl Studio {
    /// Opens the one modal the window may show. The dialog rebuilds `content` only when
    /// the window refreshes, so handlers that change what it shows call `window.refresh()`.
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
                .title(title())
                .centered()
                .entrance(DialogEntrance::None)
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
                    let _ = on_close.update(cx, |this, cx| this.modal_closed(kind, window, cx));
                })
        });
    }

    /// Closes the open modal from code. Escape and the backdrop close the dialog
    /// themselves and only report it through `modal_closed`.
    pub(super) fn close_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(kind) = self.modal {
            window.close_dialog(cx);
            self.modal_closed(kind, window, cx);
        }
    }

    /// Forgets the modal and returns focus to the control that opened it.
    fn modal_closed(&mut self, kind: Modal, window: &mut Window, cx: &mut Context<Self>) {
        if self.modal != Some(kind) {
            return;
        }
        self.modal = None;
        let trigger = match kind {
            Modal::Menu => &self.file_trigger_focus,
            Modal::Settings => &self.menu_trigger_focus,
            Modal::Model => &self.model_trigger_focus,
            Modal::Export => &self.export_trigger_focus,
        };
        trigger.focus(window, cx);
        cx.notify();
    }
}
