//! Script list composition; rows and insertion gaps are sibling elements.
use crate::workspace::view::Studio;
use crate::workspace::view::appearance::{button, tool};
use gpui_kit::component::Disableable;
use gpui_kit::component::scroll::{AutoScroll, Scrollbar, ScrollbarMode};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

#[derive(Clone)]
pub(super) struct DraggedRow {
    studio: EntityId,
    id: String,
    text: String,
}

impl Render for DraggedRow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_2()
            .rounded(px(8.))
            .bg(rgb(0x283540))
            .text_color(rgb(0xffffff))
            .text_sm()
            .shadow_lg()
            .max_w(px(320.))
            .overflow_hidden()
            .text_ellipsis()
            .child(self.text.clone())
    }
}

mod row;

impl Studio {
    pub(in crate::workspace::view) fn script(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex_1()
            .min_h_0()
            .ml_3()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.script_rows(cx))
                    .child(self.script_scrollbar()),
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
            .on_drag_move(cx.listener(|this, event: &DragMoveEvent<DraggedRow>, _, cx| {
                let delta = if event.drag(cx).studio == cx.entity_id() && !this.state.busy && this.state.draft.is_none() {
                    AutoScroll::compute_delta(event.event.position.y, event.bounds)
                } else { None };
                this.script_drag_scroll.set(delta, cx, |delta, this, cx| {
                    if !cx.has_active_drag() || this.state.busy || this.state.draft.is_some() {
                        this.script_drag_scroll.stop();
                        return;
                    }
                    let offset = this.script_scroll.offset();
                    this.script_scroll.set_offset(point(offset.x, (offset.y - delta).clamp(-this.script_scroll.max_offset().y, px(0.))));
                    cx.notify();
                });
            }))
            .p_1()
            .when(self.state.snapshot.editor.rows.is_empty(), |el| {
                let message = if self.state.status.id.is_none() {
                    "左下のファイルからプロジェクトを作成するか、開いて台本を書き始めましょう。"
                } else if self.state.snapshot.characters.is_empty() {
                    "まずは左下の＋からキャラを追加してください。追加後、台本の＋からセリフを書き始められます。"
                } else {
                    "台本がまだありません。下の＋からセリフを追加してください。"
                };
                el.child(div().ml(px(40.)).mr(px(4.)).py_4().text_sm().line_height(px(24.))
                    .text_color(self.palette().muted).child(message))
            })
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
            .child(self.script_append(cx))
            .into_any_element()
    }

    fn script_append(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy
            || self.state.draft.is_some()
            || self.state.status.id.is_none()
            || self.state.snapshot.characters.is_empty();
        div()
            .ml(px(40.))
            .mr(px(4.))
            .mb_2()
            .child(
                tool("append-script-row", "plus", "末尾にセリフを追加")
                    .w_full()
                    .h(px(32.))
                    .rounded(px(8.))
                    .bg(p.control)
                    .border_1()
                    .border_color(p.line)
                    .disabled(disabled)
                    .when(disabled, |el| el.cursor_default())
                    .on_click(cx.listener(|this, _, w, cx| {
                        let character = this.state.snapshot.editor.active_character.clone();
                        let inserted = if character.is_empty() {
                            this.state.add_row(false)
                        } else {
                            this.state.append_character_row(character)
                        };
                        if inserted {
                            this.edited(w, cx);
                            this.script_scroll.scroll_to_bottom();
                            this.fields.text.update(cx, |state, cx| state.focus(w, cx));
                        }
                    })),
            )
            .into_any_element()
    }

    fn script_insert(&self, slot: usize, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let rows = &self.state.snapshot.editor.rows;
        let after = slot.checked_sub(1).map(|i| rows[i].id.clone());
        let drop_after = after.clone();
        let disabled = self.state.busy || self.state.draft.is_some();
        let studio = cx.entity_id();
        div()
            .id(("script-insert", slot))
            .group("script-insert")
            .relative()
            .h(px(8.))
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
            .child(
                div()
                    .id(("insert-target", slot))
                    .absolute()
                    .left(px(40.))
                    .right(px(4.))
                    .top(px(-4.))
                    .h(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(!disabled, |el| {
                        el.drag_over::<DraggedRow>(move |style, drag, _, _| {
                            if drag.studio == studio {
                                style
                                    .bg(p.accent.opacity(0.25))
                                    .border_t_2()
                                    .border_color(p.accent)
                            } else {
                                style
                            }
                        })
                        .on_drop(cx.listener(
                            move |this, drag: &DraggedRow, window, cx| {
                                if drag.studio != cx.entity_id()
                                    || this.state.busy
                                    || this.state.draft.is_some()
                                {
                                    return;
                                }
                                if !this
                                    .state
                                    .snapshot
                                    .editor
                                    .rows
                                    .iter()
                                    .any(|row| row.id == drag.id)
                                {
                                    return;
                                }
                                this.select(drag.id.clone(), window, cx);
                                this.move_row(drop_after.as_deref(), cx);
                            },
                        ))
                    })
                    .child(
                        tool(("insert", slot), "plus", "ここにセリフを追加")
                            .w(px(28.))
                            .h(px(16.))
                            .bg(p.control)
                            .disabled(disabled)
                            .invisible()
                            .group_hover("script-insert", |style| style.visible())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if this.state.insert_at(after.as_deref(), false) {
                                    this.edited(window, cx);
                                    this.fields
                                        .text
                                        .update(cx, |state, cx| state.focus(window, cx));
                                }
                            })),
                    ),
            )
            .into_any_element()
    }

    fn script_scrollbar(&self) -> AnyElement {
        div()
            .relative()
            .w(px(24.))
            .flex_shrink_0()
            .child(
                Scrollbar::vertical(&self.script_scroll)
                    .viewport_from_layout()
                    .mode(ScrollbarMode::Always)
                    .styles(|s| {
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
                    }),
            )
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
