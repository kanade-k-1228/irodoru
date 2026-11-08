//! Window header: menu trigger, drag area and window controls.
use super::Studio;
use super::appearance::{icon, tool};
use gpui_kit::component::FocusableExt;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(super) fn header(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let closed = self.state.status.id.is_none();
        div()
            .flex_shrink_0()
            .bg(p.toolbar)
            .border_b_1()
            .border_color(p.line)
            .px_2()
            .py_1()
            .child(self.header_bar(window, cx))
            .when(closed, |el| {
                el.child(div().px_2().py_1().text_xs().text_color(p.muted).child(
                    if self.state.busy {
                        "プロジェクトを読み込み中…"
                    } else {
                        "プロジェクトZIPを新規作成するか、既存のZIPを開いてください。"
                    },
                ))
            })
            .into_any_element()
    }

    fn header_bar(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(self.menu_trigger(cx))
            .child(
                div()
                    .id("window-drag-area")
                    .flex_1()
                    .min_w(px(64.))
                    .h(px(32.))
                    .on_mouse_down(MouseButton::Left, |event, w, _| {
                        if event.click_count == 2 {
                            w.zoom_window();
                        } else {
                            w.start_window_move();
                        }
                    }),
            )
            .child(self.window_controls(window, cx))
            .into_any_element()
    }

    fn menu_trigger(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        Button::new("app-menu-trigger")
            .track_focus(&self.menu_trigger_focus)
            .focus_ring(false)
            .ghost()
            .px_1()
            .py_1()
            .h(px(32.))
            .rounded(px(8.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_base()
                    .font_weight(FontWeight::BOLD)
                    .child(icon("audio").size(px(20.)).text_color(p.accent))
                    .child("irodoru"),
            )
            .on_click(cx.listener(|this, _, w, cx| {
                if this.menu_open {
                    this.menu_open = false;
                    this.menu_trigger_focus.focus(w, cx);
                } else {
                    this.open_menu(false, w, cx);
                }
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, w, cx| {
                if ["up", "down"].contains(&event.keystroke.key.as_str()) {
                    this.open_menu(event.keystroke.key == "up", w, cx);
                    cx.stop_propagation();
                }
            }))
            .into_any_element()
    }

    fn window_controls(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_shrink_0()
            .my(px(-4.))
            .mr(px(-8.))
            .child(
                tool("minimize", "minus", "最小化")
                    .w(px(44.))
                    .h(px(40.))
                    .rounded_none()
                    .on_click(|_, w, _| w.minimize_window()),
            )
            .child(
                tool(
                    "maximize",
                    if window.is_maximized() {
                        "copy"
                    } else {
                        "square"
                    },
                    if window.is_maximized() {
                        "元のサイズに戻す"
                    } else {
                        "最大化"
                    },
                )
                .w(px(44.))
                .h(px(40.))
                .rounded_none()
                .on_click(|_, w, _| w.zoom_window()),
            )
            .child(
                gpui_kit::base::Button::new("close-window")
                    .accessibility_label("閉じる")
                    .child(icon("close"))
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(44.))
                    .h(px(40.))
                    .rounded_none()
                    .hover(|s| s.bg(rgb(0xc42b1c)).text_color(rgb(0xffffff)))
                    .disabled(self.state.busy)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.close();
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
