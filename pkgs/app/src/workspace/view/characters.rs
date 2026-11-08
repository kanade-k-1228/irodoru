use super::Studio;
use super::appearance::{character_color, tool};
use crate::workspace::editor;
use gpui_kit::component::Disableable;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(super) fn characters(&self, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
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
                    .children(
                        self.state
                            .snapshot
                            .characters
                            .iter()
                            .enumerate()
                            .map(|(i, c)| {
                                let id = c.id.clone();
                                let selected = self
                                    .state
                                    .draft
                                    .as_ref()
                                    .and_then(|d| d.id.as_ref())
                                    .unwrap_or(&self.state.snapshot.editor.active_character)
                                    == &id;
                                let color = character_color(Some(&c.color), p.accent);
                                div()
                                    .id(("character", i))
                                    .relative()
                                    .min_h(px(44.))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .px_2()
                                    .border_1()
                                    .border_color(if selected { p.line } else { p.surface })
                                    .border_l(px(2.))
                                    .when(selected, |el| el.bg(p.panel))
                                    .child(
                                        div()
                                            .absolute()
                                            .left_0()
                                            .top_0()
                                            .bottom_0()
                                            .w(px(2.))
                                            .bg(if selected { color } else { p.surface }),
                                    )
                                    .text_color(if selected { p.ink } else { p.muted })
                                    .cursor_pointer()
                                    .hover(|s| s.bg(p.control))
                                    .child(self.avatar(Some(c), 32.))
                                    .when(!narrow, |el| {
                                        el.child(
                                            div().text_xs().overflow_hidden().child(c.name.clone()),
                                        )
                                    })
                                    .when(!disabled, |el| {
                                        el.on_click(cx.listener(move |this, _, w, cx| {
                                            this.state.snapshot.editor.active_character =
                                                id.clone();
                                            match editor::insert(
                                                &mut this.state.snapshot.editor,
                                                None,
                                                false,
                                            ) {
                                                Ok(id) => this.select(id, w, cx),
                                                Err(e) => {
                                                    this.state.error = Some(e.to_string());
                                                    cx.notify();
                                                }
                                            }
                                        }))
                                    })
                            }),
                    ),
            )
            .child(
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
                            .on_click(
                                cx.listener(|this, _, w, cx| this.edit_character(None, w, cx)),
                            ),
                    )
                    .child(
                        tool("edit-character", "edit", "キャラを編集")
                            .size(px(32.))
                            .disabled(disabled || active.is_none())
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.edit_character(active.clone(), w, cx)
                            })),
                    ),
            )
            .into_any_element()
    }
}
