use super::appearance::{icon, tool};
use super::{Studio, WindowRequest, appearance};
use crate::workspace::protocol::Command;
use gpui_kit::component::WindowExt;
use gpui_kit::component::button::Button;
use gpui_kit::component::button::ButtonVariants;
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

const LABELS: [&str; 8] = [
    "新規ウィンドウ",
    "新規プロジェクト…",
    "プロジェクトを開く…",
    "プロジェクトを保存",
    "ダークテーマ",
    "環境設定…",
    "irodoruについて",
    "ウィンドウを閉じる",
];
impl Studio {
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
    fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.light = crate::preferences::theme::toggle(cx);
        Theme::change(
            if self.settings.light {
                ThemeMode::Light
            } else {
                ThemeMode::Dark
            },
            Some(window),
            cx,
        );
        appearance::apply(self.settings.light, cx);
        if let Err(error) = crate::preferences::theme::save(cx, self.settings.light) {
            self.state.error = Some(error.to_string());
        }
        cx.notify();
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
                    .left(px(if self.settings.light { 2. } else { 26. }))
                    .size(px(18.))
                    .rounded_full()
                    .bg(p.ink),
            )
            .child(
                icon("sun")
                    .size(px(14.))
                    .text_color(if self.settings.light {
                        p.surface
                    } else {
                        p.muted
                    }),
            )
            .child(
                icon("moon")
                    .size(px(14.))
                    .text_color(if self.settings.light {
                        p.muted
                    } else {
                        p.surface
                    }),
            )
            .into_any_element()
    }
    fn show_app_dialog(&mut self, preferences: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog_open = true;
        let weak = cx.entity().downgrade();
        let on_close = weak.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let content = weak
                .update(cx, |this, cx| {
                    let p = this.palette();
                    if preferences {
                        div()
                            .mt_5()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_8()
                            .child("ダークテーマ")
                            .child(
                                Button::new("preferences-theme")
                                    .ghost()
                                    .child(this.theme_track())
                                    .on_click(
                                        cx.listener(|this, _, w, cx| this.toggle_theme(w, cx)),
                                    ),
                            )
                            .into_any_element()
                    } else {
                        div()
                            .mt_5()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .text_sm()
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
                        .child(if preferences {
                            "環境設定"
                        } else {
                            "irodoruについて"
                        })
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
                .overlay_closable(false)
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
    fn open_menu(&mut self, last: bool, window: &mut Window, cx: &mut Context<Self>) {
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
    pub(super) fn header(
        &self,
        inference: String,
        status: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let closed = self.state.status.id.is_none();
        div()
            .flex_shrink_0()
            .bg(p.toolbar)
            .border_b_1()
            .border_color(p.line)
            .px_2()
            .py_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("app-menu-trigger")
                            .track_focus(&self.menu_trigger_focus)
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
                                    .child("irodoru")
                                    .child(icon("chevron-down").size(px(12.)).text_color(p.muted)),
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
                            })),
                    )
                    .child(
                        div()
                            .id("project-title")
                            .flex_1()
                            .min_w(px(64.))
                            .h(px(32.))
                            .flex()
                            .items_center()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_xs()
                            .text_color(p.muted)
                            .child(if closed {
                                "プロジェクト未選択".into()
                            } else {
                                format!(
                                    "{} · {status}",
                                    self.state.status.name.as_deref().unwrap_or_default()
                                )
                            })
                            .on_mouse_down(MouseButton::Left, |event, w, _| {
                                if event.click_count == 2 {
                                    w.zoom_window();
                                } else {
                                    w.start_window_move();
                                }
                            }),
                    )
                    .child(
                        div()
                            .max_w(px(256.))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_xs()
                            .text_color(p.muted)
                            .child(inference),
                    )
                    .child(
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
                            ),
                    ),
            )
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
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, w, cx| {
                    let key = event.keystroke.key.as_str();
                    match key {
                        "escape" | "tab" => {
                            this.menu_open = false;
                            this.menu_trigger_focus.focus(w, cx);
                        }
                        "enter" | "space" => this.menu_select(this.menu_index, w, cx),
                        "home" => this.menu_index = 0,
                        "end" => {
                            this.menu_index = (0..LABELS.len())
                                .rev()
                                .find(|i| !this.menu_disabled(*i))
                                .unwrap_or(0)
                        }
                        "up" | "down" => {
                            let delta = if key == "up" { LABELS.len() - 1 } else { 1 };
                            for _ in 0..LABELS.len() {
                                this.menu_index = (this.menu_index + delta) % LABELS.len();
                                if !this.menu_disabled(this.menu_index) {
                                    break;
                                }
                            }
                        }
                        _ => return,
                    };
                    cx.stop_propagation();
                    cx.notify();
                }))
                .children(LABELS.iter().enumerate().map(|(index, label)| {
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
                                .border_2()
                                .border_color(gpui::transparent_black())
                                .px(px(10.))
                                .py(px(6.))
                                .text_sm()
                                .line_height(px(20.))
                                .text_color(p.ink)
                                .when(disabled, |el| el.opacity(0.4))
                                .when(!disabled, |el| el.cursor_pointer().hover(|s| s.bg(p.hover)))
                                .when(self.menu_index == index, |el| {
                                    el.bg(p.hover).border_color(p.accent)
                                })
                                .child(*label)
                                .when(index == 4, |el| el.child(self.theme_track()))
                                .on_click(cx.listener(move |this, _, w, cx| {
                                    this.menu_select(index, w, cx)
                                })),
                        )
                })),
        )
        .with_priority(10)
        .into_any_element()
    }
}
