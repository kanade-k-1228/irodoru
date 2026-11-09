//! Menu layout and navigation; header and dialog layouts live in sibling modules.
use super::appearance::{icon, tool};
use super::dialogs::ProjectDialog;
use super::{Studio, WindowRequest, actions, appearance};
use crate::workspace::protocol::Command;
use gpui_kit::component::{Theme as ComponentTheme, ThemeMode};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuAction {
    NewWindow,
    NewProject,
    NewFromCharacters,
    ImportCharacter,
    OpenProject,
    SaveProject,
    Preferences,
    CloseWindow,
}
use MenuAction::*;

const MENU_ITEMS: [(MenuAction, &str, &str); 7] = [
    (NewWindow, "新規ウィンドウ", "app-window"),
    (NewProject, "新規プロジェクト…", "new"),
    (
        NewFromCharacters,
        "キャラクターを引き継いで新規作成…",
        "copy",
    ),
    (ImportCharacter, "キャラクターファイルを読み込む…", "folder"),
    (OpenProject, "プロジェクトを開く…", "folder"),
    (SaveProject, "プロジェクトを保存", "save"),
    (CloseWindow, "ウィンドウを閉じる", "close"),
];

impl Studio {
    pub(super) fn app_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        deferred(
            div()
                .id("app-menu-modal")
                .absolute()
                .inset_0()
                .occlude()
                .cursor_default()
                .flex()
                .items_center()
                .justify_center()
                .bg(if self.settings.theme.is_light() {
                    rgba(0x192c443d)
                } else {
                    rgba(0x0000008c)
                })
                .track_focus(&self.menu_focus)
                .key_context("AppMenu")
                .on_action(cx.listener(|this, _: &actions::MenuNext, _, cx| {
                    this.move_menu_selection(false);
                    cx.stop_propagation();
                    cx.notify();
                }))
                .on_action(cx.listener(|this, _: &actions::MenuPrevious, _, cx| {
                    this.move_menu_selection(true);
                    cx.stop_propagation();
                    cx.notify();
                }))
                .capture_key_down(cx.listener(Self::menu_key_down))
                .child(
                    div()
                        .id("app-menu-backdrop")
                        .absolute()
                        .inset_0()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| this.close_menu(window, cx)),
                        ),
                )
                .child(
                    div()
                        .id("app-menu")
                        .relative()
                        .occlude()
                        .cursor_default()
                        .w(px(384.))
                        .max_w_full()
                        .mx_4()
                        .rounded(px(12.))
                        .border_1()
                        .border_color(p.line)
                        .bg(p.panel)
                        .p_3()
                        .shadow_xl()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .px_3()
                                .pb_2()
                                .child("ファイル")
                                .child(tool("menu-close", "close", "ファイルを閉じる").on_click(
                                    cx.listener(|this, _, window, cx| this.close_menu(window, cx)),
                                )),
                        )
                        .children(MENU_ITEMS.iter().enumerate().map(
                            |(index, (_, label, icon_name))| {
                                self.menu_item(index, label, icon_name, cx)
                            },
                        )),
                ),
        )
        .with_priority(10)
        .into_any_element()
    }

    fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu_open = false;
        self.file_trigger_focus.focus(window, cx);
        cx.notify();
    }

    fn menu_item(
        &self,
        index: usize,
        label: &'static str,
        icon_name: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let disabled = self.menu_disabled(index);
        div()
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
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(icon(icon_name).flex_shrink_0())
                            .child(label),
                    )
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.menu_select(MENU_ITEMS[index].0, w, cx)
                    })),
            )
            .into_any_element()
    }

    pub(super) fn open_project_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog_open {
            return;
        }
        self.menu_open = true;
        self.menu_index = (0..MENU_ITEMS.len())
            .find(|i| !self.menu_disabled(*i))
            .unwrap_or(0);
        self.menu_focus.focus(window, cx);
        cx.notify();
    }

    fn menu_key_down(&mut self, event: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        match key {
            "escape" => self.close_menu(w, cx),
            "enter" | "space" => self.menu_select(MENU_ITEMS[self.menu_index].0, w, cx),
            "home" => {
                self.menu_index = (0..MENU_ITEMS.len())
                    .find(|i| !self.menu_disabled(*i))
                    .unwrap_or(0);
            }
            "end" => {
                self.menu_index = (0..MENU_ITEMS.len())
                    .rev()
                    .find(|i| !self.menu_disabled(*i))
                    .unwrap_or(0)
            }
            "up" | "down" => self.move_menu_selection(key == "up"),
            _ => return,
        };
        cx.stop_propagation();
        cx.notify();
    }

    fn move_menu_selection(&mut self, backwards: bool) {
        let delta = if backwards { MENU_ITEMS.len() - 1 } else { 1 };
        for _ in 0..MENU_ITEMS.len() {
            self.menu_index = (self.menu_index + delta) % MENU_ITEMS.len();
            if !self.menu_disabled(self.menu_index) {
                break;
            }
        }
    }

    fn menu_disabled(&self, index: usize) -> bool {
        self.action_disabled(MENU_ITEMS[index].0)
    }
    fn action_disabled(&self, action: MenuAction) -> bool {
        match action {
            NewProject | OpenProject => self.state.busy || self.state.draft.is_some(),
            NewFromCharacters | ImportCharacter => {
                self.state.busy || self.state.draft.is_some() || self.state.status.id.is_none()
            }
            SaveProject => self.state.busy || self.state.status.id.is_none(),
            CloseWindow => self.state.busy,
            _ => false,
        }
    }

    pub(super) fn menu_select(
        &mut self,
        action: MenuAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.dialog_open || self.action_disabled(action) {
            return;
        }
        if self.menu_open {
            self.menu_open = false;
            self.file_trigger_focus.focus(window, cx);
        }
        match action {
            NewWindow => {
                cx.emit(WindowRequest::New);
            }
            NewProject => self.choose_project(ProjectDialog::New, window, cx),
            NewFromCharacters => self.choose_project(ProjectDialog::FromCharacters, window, cx),
            ImportCharacter => self.character_file(false, window, cx),
            OpenProject => self.choose_project(ProjectDialog::Open, window, cx),
            SaveProject => {
                self.autosave();
                self.send(Command::Flush { close: false });
            }
            Preferences => self.show_settings(window, cx),
            CloseWindow => self.close(),
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
        ComponentTheme::change(
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
