//! Script list composition; rows and insertion gaps are sibling elements.
use super::Studio;
use super::appearance::{button, character_color, tool};
use crate::domain::ScriptRow;
use gpui_kit::component::Disableable;
use gpui_kit::component::scroll::{Scrollbar, ScrollbarMode};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(super) fn script(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex_1()
            .min_h_0()
            .mx_3()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.script_rows(cx))
                    .child(self.script_navigation(cx)),
            )
            .when(self.state.running.is_some(), |el| {
                el.child(self.generation_stop(cx))
            })
            .into_any_element()
    }

    fn script_rows(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("script")
            .flex_1()
            .min_w_0()
            .overflow_y_scroll()
            .track_scroll(&self.script_scroll)
            .p_1()
            .when(!self.state.snapshot.editor.rows.is_empty(), |el| {
                el.child(self.script_insert(0, cx))
            })
            .children(
                self.state
                    .snapshot
                    .editor
                    .rows
                    .iter()
                    .enumerate()
                    .flat_map(|(i, row)| {
                        [self.script_row(i, row, cx), self.script_insert(i + 1, cx)]
                    }),
            )
            .into_any_element()
    }

    fn script_row(&self, i: usize, row: &ScriptRow, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let id = row.id.clone();
        let selected = self.state.selected.as_ref() == Some(&id);
        let running = self.state.running.as_ref() == Some(&id);
        let character = self
            .state
            .snapshot
            .characters
            .iter()
            .find(|c| c.id == row.character_id);
        let color = character_color(character.map(|c| c.color.as_str()), p.accent);
        let last = i + 1 == self.state.snapshot.editor.rows.len();
        div()
            .id(("row", i))
            .relative()
            .rounded(px(8.))
            .px_1()
            .py_1()
            .when(running, |el| el.border_1().border_color(p.accent))
            // Continue the rail through tall rows, behind the opaque avatars.
            .when(self.state.snapshot.editor.rows.len() > 1, |el| {
                el.child(
                    div()
                        .absolute()
                        .left(px(18.))
                        .top(px(if i == 0 { 24. } else { 0. }))
                        .w(px(1.))
                        .bg(p.line)
                        .when(last, |el| el.h(px(24.)))
                        .when(!last, |el| el.bottom_0()),
                )
            })
            .child(
                div()
                    .relative()
                    .flex()
                    .items_start()
                    .gap_2()
                    .child(
                        self.avatar(character, 28.)
                            // Match the speech field's top border and padding.
                            .mt(px(6.))
                            .when(selected, |el| el.border_2().border_color(color)),
                    )
                    .child(self.script_speech(i, row, selected, color, cx)),
            )
            .when_some(self.state.row_errors.get(&id).cloned(), |el, error| {
                el.child(div().text_xs().text_color(p.warning).child(error))
            })
            .into_any_element()
    }

    fn script_remove_button(&self, i: usize, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.state.busy
            || self.state.draft.is_some()
            || self.state.snapshot.editor.rows.len() <= 1;
        tool(("remove", i), "trash", "選択行を削除")
            .flex_shrink_0()
            .bg(rgb(0xb52e42))
            .text_color(rgb(0xffffff))
            .disabled(disabled)
            .when(disabled, |el| el.cursor_default())
            .on_click(cx.listener(|this, _, w, cx| {
                cx.stop_propagation();
                this.remove_row(w, cx);
            }))
            .into_any_element()
    }

    fn script_speech(
        &self,
        i: usize,
        row: &ScriptRow,
        selected: bool,
        color: Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let select = row.id.clone();
        div()
            .id(("speech", i))
            .line_height(px(24.))
            .flex()
            .items_center()
            .gap_2()
            .flex_1()
            .min_w_0()
            .min_h(px(40.))
            .rounded(px(8.))
            .border_2()
            .border_color(if selected { color } else { p.line })
            .bg(p.control)
            .px_2()
            .py_1()
            .text_sm()
            .text_color(if row.text.is_empty() { p.muted } else { p.ink })
            .cursor_pointer()
            .hover(|s| s.bg(p.hover))
            .child(div().flex_1().min_w_0().child(if row.text.is_empty() {
                "セリフを入力".to_owned()
            } else {
                row.text.clone()
            }))
            .when(selected, |el| el.child(self.script_remove_button(i, cx)))
            .on_click(cx.listener(move |this, _, w, cx| {
                this.select(select.clone(), w, cx);
                this.fields.text.update(cx, |state, cx| state.focus(w, cx));
            }))
            .into_any_element()
    }

    fn script_insert(&self, slot: usize, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let rows = &self.state.snapshot.editor.rows;
        let after = slot.checked_sub(1).map(|i| rows[i].id.clone());
        let disabled = self.state.busy || self.state.draft.is_some();
        let can_move = !disabled
            && self.state.selected.as_ref().is_some_and(|id| {
                rows.iter()
                    .position(|row| &row.id == id)
                    .is_some_and(|index| slot != index && slot != index + 1)
            });
        div()
            .id(("script-insert", slot))
            .group("script-insert")
            .relative()
            .h(px(20.))
            .when(slot > 0 && slot < rows.len(), |el| {
                el.child(
                    div()
                        .absolute()
                        .left(px(18.))
                        .top_0()
                        .h_full()
                        .w(px(1.))
                        .bg(p.line),
                )
            })
            .when(can_move, |el| {
                let after = after.clone();
                el.child(
                    div()
                        .absolute()
                        .left(px(8.))
                        .top_0()
                        .invisible()
                        .group_hover("script-insert", |style| style.visible())
                        .child(
                            tool(("move-here", slot), "chevrons-right", "ここに挿入")
                                .size(px(20.))
                                .rounded(px(4.))
                                .bg(p.panel)
                                .border_1()
                                .border_color(p.line)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.move_row(after.as_deref(), cx);
                                })),
                        ),
                )
            })
            .when(!disabled, |el| {
                el.when_some(after, |el, after| {
                    el.child(
                        div()
                            .absolute()
                            .left(px(40.))
                            .right(px(4.))
                            .top_0()
                            .h_full()
                            .invisible()
                            .group_hover("script-insert", |style| style.visible())
                            .child(
                                tool(("insert", slot), "plus", "ここにセリフを追加")
                                    .w_full()
                                    .h_full()
                                    .rounded(px(4.))
                                    .bg(p.control)
                                    .border_1()
                                    .border_color(p.line)
                                    .on_click(cx.listener(move |this, _, w, cx| {
                                        if this.state.insert_after(after.clone()) {
                                            this.edited(w, cx);
                                        }
                                    })),
                            ),
                    )
                })
            })
            .into_any_element()
    }

    fn script_navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .w(px(24.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_center()
            .child(
                tool("scroll-top", "up", "台本の先頭へ")
                    .size(px(24.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.script_scroll.set_offset(point(px(0.), px(0.)));
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .w(px(24.))
                    .child(self.script_scrollbar())
                    .when(self.script_scroll.max_offset().y > px(0.), |el| {
                        // Override the scrollbar's arrow cursor without intercepting its events.
                        el.child(div().absolute().inset_0().cursor_pointer())
                    }),
            )
            .child(
                tool("scroll-bottom", "down", "台本の末尾へ")
                    .size(px(24.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.script_scroll.scroll_to_bottom();
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn script_scrollbar(&self) -> AnyElement {
        if self.script_scroll.max_offset().y <= px(0.) {
            div()
                .w(px(4.))
                .h_full()
                .mx_auto()
                .rounded_full()
                .child(div().w(px(12.)).h(px(40.)).ml(px(-4.)).rounded(px(6.)).bg(
                    if self.settings.theme.is_light() {
                        rgb(0x9ba8b8)
                    } else {
                        rgb(0x397cae)
                    },
                ))
                .into_any_element()
        } else {
            Scrollbar::vertical(&self.script_scroll)
                .viewport_from_layout()
                .mode(ScrollbarMode::Always)
                .styles(|s| {
                    // The track also defines the pointer hitbox; cover the whole thumb.
                    s.track(|s| s.bg(transparent_black()).width(px(24.)))
                        .thumb(|s| {
                            s.bg(if self.settings.theme.is_light() {
                                rgb(0x9ba8b8)
                            } else {
                                rgb(0x397cae)
                            })
                            .width(px(12.))
                            .inset(px(6.))
                        })
                })
                .into_any_element()
        }
        .into_any_element()
    }

    fn generation_stop(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .p_1()
            .child(
                button(
                    "stop",
                    if self.state.stopping {
                        "停止待ち"
                    } else {
                        "停止"
                    },
                )
                .disabled(self.state.stopping)
                .when(self.state.stopping, |el| el.cursor_default())
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Some(s) = &this.service {
                        s.stop();
                        this.state.stopping = true;
                    }
                    cx.notify();
                })),
            )
            .into_any_element()
    }
}
