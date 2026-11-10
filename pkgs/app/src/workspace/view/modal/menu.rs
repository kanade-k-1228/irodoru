//! The project menu modal: its items, keyboard navigation and the actions it runs.
use super::Modal;
use crate::workspace::protocol::Command;
use crate::workspace::view::appearance::icon;
use crate::workspace::view::dialogs::ProjectDialog;
use crate::workspace::view::{Studio, WindowRequest, actions};
use gpui_kit::base::actions::Confirm;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::workspace::view) enum MenuAction {
    NewWindow,
    NewProject,
    NewFromCharas,
    ImportChara,
    OpenProject,
    SaveProject,
    Preferences,
    CloseWindow,
}
use MenuAction::*;

const MENU_ITEMS: [(MenuAction, &str, &str); 7] = [
    (NewWindow, "新規ウィンドウ", "app-window"),
    (NewProject, "新規プロジェクト…", "new"),
    (NewFromCharas, "キャラクターを引き継いで新規作成…", "copy"),
    (ImportChara, "キャラクターファイルを読み込む…", "folder"),
    (OpenProject, "プロジェクトを開く…", "folder"),
    (SaveProject, "プロジェクトを保存", "save"),
    (CloseWindow, "ウィンドウを閉じる", "close"),
];

impl Studio {
    pub(in crate::workspace::view) fn open_project_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.modal.is_some() {
            return;
        }
        self.menu_index = (0..MENU_ITEMS.len())
            .find(|i| !self.menu_disabled(*i))
            .unwrap_or(0);
        self.open_modal(
            Modal::Menu,
            384.,
            || "ファイル".into_any_element(),
            |this, cx| this.menu_content(cx),
            window,
            cx,
        );
        self.menu_focus.focus(window, cx);
    }

    fn menu_content(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("app-menu")
            .mt_3()
            .track_focus(&self.menu_focus)
            .key_context("AppMenu")
            .on_action(cx.listener(|this, _: &actions::MenuNext, window, _| {
                this.move_menu_selection(false);
                window.refresh();
            }))
            .on_action(cx.listener(|this, _: &actions::MenuPrevious, window, _| {
                this.move_menu_selection(true);
                window.refresh();
            }))
            // The dialog binds Enter to its own confirmation, which only closes it.
            .on_action(cx.listener(|this, _: &Confirm, window, cx| {
                this.menu_select(MENU_ITEMS[this.menu_index].0, window, cx);
            }))
            .capture_key_down(cx.listener(Self::menu_key_down))
            .children(
                MENU_ITEMS
                    .iter()
                    .enumerate()
                    .map(|(index, (_, label, icon_name))| {
                        self.menu_item(index, label, icon_name, cx)
                    }),
            )
            .into_any_element()
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

    fn menu_key_down(&mut self, event: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        match key {
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
        w.refresh();
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
            NewProject | OpenProject => self.state.busy,
            NewFromCharas | ImportChara => self.state.busy || self.state.status.id.is_none(),
            SaveProject => self.state.busy || self.state.status.id.is_none(),
            CloseWindow => self.state.busy,
            _ => false,
        }
    }

    pub(in crate::workspace::view) fn menu_select(
        &mut self,
        action: MenuAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.action_disabled(action) {
            return;
        }
        match self.modal {
            Some(Modal::Menu) => self.close_modal(window, cx),
            Some(_) => return,
            None => {}
        }
        match action {
            NewWindow => {
                cx.emit(WindowRequest::New);
            }
            NewProject => self.choose_project(ProjectDialog::New, window, cx),
            NewFromCharas => self.choose_project(ProjectDialog::FromCharas, window, cx),
            ImportChara => self.chara_file(false, window, cx),
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
}
