use super::Studio;
use super::appearance::tool;
use crate::domain::Character;
use gpui_kit::component::Disableable;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(super) fn characters(&self, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        div()
            .w(px(if narrow { 56. } else { 208. }))
            .flex_shrink_0()
            .min_h_0()
            .border_r_1()
            .border_color(p.line)
            .bg(p.surface)
            .flex()
            .flex_col()
            .child(
                div()
                    .id("characters")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_1()
                    .when(self.state.snapshot.characters.is_empty(), |el| {
                        let message = if self.state.status.id.is_none() {
                            if narrow {
                                "左下からファイルを開く"
                            } else {
                                "左下のファイルからプロジェクトを作成するか、開いてください。"
                            }
                        } else if self.state.draft.is_some() {
                            if narrow {
                                "保存で追加"
                            } else {
                                "キャラの名前と設定を入力し、保存すると一覧に追加されます。"
                            }
                        } else if narrow {
                            "下の＋で追加"
                        } else {
                            "キャラがまだいません。下の＋から、名前や声の設定を追加してください。"
                        };
                        el.child(
                            div()
                                .px_1()
                                .py_3()
                                .text_xs()
                                .line_height(px(20.))
                                .text_color(p.muted)
                                .child(message),
                        )
                    })
                    .children(
                        self.state
                            .snapshot
                            .characters
                            .iter()
                            .enumerate()
                            .map(|(i, c)| self.character_item(i, c, narrow, cx)),
                    ),
            )
            .child(self.character_list_actions(narrow, cx))
            .into_any_element()
    }

    fn character_item(
        &self,
        i: usize,
        c: &Character,
        narrow: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let disabled =
            self.state.busy || self.state.draft.is_some() || self.state.status.id.is_none();

        let id = c.id.clone();
        let selected = self
            .state
            .draft
            .as_ref()
            .and_then(|d| d.id.as_ref())
            .unwrap_or(&self.state.snapshot.editor.active_character)
            == &id;
        div()
            .id(("character", i))
            .min_h(px(44.))
            .mb_1()
            .rounded(px(8.))
            .flex()
            .items_center()
            .gap_2()
            .px(px(if narrow { 4. } else { 8. }))
            .when(selected, |el| el.bg(p.control))
            .text_color(if selected { p.ink } else { p.muted })
            .when(!disabled, |el| el.cursor_pointer().hover(|s| s.bg(p.hover)))
            .child(self.avatar(Some(c), 32.))
            .when(!narrow, |el| {
                el.child(div().text_xs().overflow_hidden().child(c.name.clone()))
            })
            .when(!disabled, |el| {
                el.on_click(cx.listener(move |this, _, w, cx| {
                    if this.state.select_character(id.clone()) {
                        this.edited(w, cx);
                    }
                }))
            })
            .into_any_element()
    }

    fn character_list_actions(&self, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled =
            self.state.busy || self.state.draft.is_some() || self.state.status.id.is_none();
        let active = self
            .state
            .snapshot
            .characters
            .iter()
            .find(|c| c.id == self.state.snapshot.editor.active_character)
            .cloned();
        let no_active = active.is_none();
        div()
            .flex()
            .gap_1()
            .when(narrow, |el| el.flex_col().items_center())
            .px_2()
            .py_1()
            .border_t_1()
            .border_color(p.line)
            .child(
                tool("add-character", "plus", "キャラを追加")
                    .size(px(32.))
                    .disabled(disabled)
                    .when(disabled, |el| el.cursor_default())
                    .on_click(cx.listener(|this, _, w, cx| this.edit_character(None, w, cx))),
            )
            .child(
                tool("edit-character", "edit", "キャラを編集")
                    .size(px(32.))
                    .disabled(disabled || active.is_none())
                    .when(disabled || active.is_none(), |el| el.cursor_default())
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.edit_character(active.clone(), w, cx)
                    })),
            )
            .child(
                tool("duplicate-character", "copy", "キャラを複製")
                    .size(px(32.))
                    .disabled(disabled || no_active)
                    .when(disabled || no_active, |el| el.cursor_default())
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(command) = this.state.duplicate_character() {
                            this.send(command);
                            cx.notify();
                        }
                    })),
            )
            .into_any_element()
    }
}
