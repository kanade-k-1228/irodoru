//! Menu layout and navigation; header and dialog layouts live in sibling modules.
use super::appearance::icon;
use super::{Studio, WindowRequest, appearance};
use crate::workspace::protocol::Command;
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

const LABELS: [&str; 8] = [
    "新規ウィンドウ",
    "新規プロジェクト…",
    "プロジェクトを開く…",
    "プロジェクトを保存",
    "カラーテーマ",
    "環境設定…",
    "irodoruについて",
    "ウィンドウを閉じる",
];

impl Studio {
    pub(super) fn app_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        deferred(
            div()
                .id("app-menu")
                .absolute()
                .left(px(8.))
                .top(px(44.))
                .w(px(256.))
                .rounded(px(8.))
                .border_1()
                .border_color(p.line)
                .bg(p.panel)
                .p_1()
                .shadow_xl()
                .track_focus(&self.menu_focus)
                .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    // The trigger toggles the menu itself; do not close and reopen on the same click.
                    if event.position.y < px(40.) && event.position.x < px(140.) {
                        return;
                    }
                    this.menu_open = false;
                    cx.notify();
                }))
                .on_key_down(cx.listener(Self::menu_key_down))
                .children(
                    LABELS
                        .iter()
                        .enumerate()
                        .map(|(index, label)| self.menu_item(index, label, cx)),
                ),
        )
        .with_priority(10)
        .into_any_element()
    }

    fn menu_item(&self, index: usize, label: &'static str, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.menu_disabled(index);
        div()
            .when(index == 4 || index == 6, |el| {
                el.child(div().h(px(1.)).my_1().bg(p.line))
            })
            .child(
                div()
                    .id(("menu-item", index))
                    .min_h(px(36.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .rounded(px(8.))
                    .px(px(12.))
                    .py(px(8.))
                    .text_sm()
                    .line_height(px(20.))
                    .text_color(p.ink)
                    .when(disabled, |el| el.opacity(0.4))
                    .when(!disabled, |el| el.cursor_pointer().hover(|s| s.bg(p.hover)))
                    .when(self.menu_index == index, |el| el.bg(p.hover))
                    .child(label)
                    .when(index == 4, |el| el.child(self.theme_track()))
                    .on_click(cx.listener(move |this, _, w, cx| this.menu_select(index, w, cx))),
            )
            .into_any_element()
    }

    pub(super) fn open_menu(&mut self, last: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.menu_open = true;
        self.menu_index = if last {
            (0..LABELS.len())
                .rev()
                .find(|i| !self.menu_disabled(*i))
                .unwrap_or(0)
        } else {
            0
        };
        self.menu_focus.focus(window, cx);
        cx.notify();
    }

    fn menu_key_down(&mut self, event: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        match key {
            "escape" | "tab" => {
                self.menu_open = false;
                self.menu_trigger_focus.focus(w, cx);
            }
            "enter" | "space" => self.menu_select(self.menu_index, w, cx),
            "home" => self.menu_index = 0,
            "end" => {
                self.menu_index = (0..LABELS.len())
                    .rev()
                    .find(|i| !self.menu_disabled(*i))
                    .unwrap_or(0)
            }
            "up" | "down" => {
                let delta = if key == "up" { LABELS.len() - 1 } else { 1 };
                for _ in 0..LABELS.len() {
                    self.menu_index = (self.menu_index + delta) % LABELS.len();
                    if !self.menu_disabled(self.menu_index) {
                        break;
                    }
                }
            }
            _ => return,
        };
        cx.stop_propagation();
        cx.notify();
    }

    fn menu_disabled(&self, index: usize) -> bool {
        match index {
            1 | 2 => self.state.busy || self.state.draft.is_some(),
            3 => self.state.busy || self.state.status.id.is_none(),
            7 => self.state.busy,
            _ => false,
        }
    }

    pub(super) fn menu_select(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.dialog_open || self.menu_disabled(index) {
            return;
        }
        if index != 4 && self.menu_open {
            self.menu_open = false;
            self.menu_trigger_focus.focus(window, cx);
        }
        match index {
            0 => {
                cx.emit(WindowRequest::New);
            }
            1 => self.choose_project(true, window, cx),
            2 => self.choose_project(false, window, cx),
            3 => {
                self.autosave();
                self.send(Command::Flush { close: false }, true);
            }
            4 => self.toggle_theme(window, cx),
            5 | 6 => self.show_app_dialog(index == 5, window, cx),
            7 => self.close(),
            _ => {}
        }
        cx.notify();
    }

    pub(super) fn theme_track(&self) -> AnyElement {
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

    pub(super) fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.theme = crate::preferences::theme::toggle(cx);
        Theme::change(
            if self.settings.theme.is_light() {
                ThemeMode::Light
            } else {
                ThemeMode::Dark
            },
            Some(window),
            cx,
        );
        appearance::apply(self.settings.theme.is_light(), cx);
        if let Err(error) = crate::preferences::theme::save(cx, self.settings.theme) {
            self.state.error = Some(error.to_string());
        }
        cx.notify();
    }
}
