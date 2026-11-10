//! Line list composition; lines and insertion gaps are sibling elements.
use crate::workspace::view::Studio;
use crate::workspace::view::appearance::DisabledWithCursor;
use crate::workspace::view::appearance::{button, icon_button, tool};
use gpui_kit::component::Disableable;
use gpui_kit::component::scroll::{AutoScroll, Scrollbar, ScrollbarMode};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

#[derive(Clone)]
pub(super) struct DraggedLine {
    studio: EntityId,
    id: String,
    text: String,
}

impl Render for DraggedLine {
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

mod actions;
mod editor;
mod line;
mod takes;

impl Studio {
    pub(in crate::workspace::view) fn lines(&self, cx: &mut Context<Self>) -> AnyElement {
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
                    .child(self.line_wheel.probe())
                    .child(self.line_list(cx))
                    .child(self.line_scrollbar()),
            )
            .when(self.state.running.is_some(), |el| {
                el.child(self.generation_stop(cx))
            })
            .child(
                div()
                    .flex_shrink_0()
                    .p_1()
                    .mr(px(24.))
                    .child(self.line_append(cx)),
            )
            .into_any_element()
    }

    fn line_list(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("lines")
            .flex_1()
            .min_w_0()
            .overflow_y_scroll()
            .track_scroll(&self.line_scroll)
            .on_scroll(self.line_wheel.listener(cx.entity_id()))
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<DraggedLine>, _, cx| {
                    let delta = if event.drag(cx).studio == cx.entity_id() && !this.state.busy {
                        AutoScroll::compute_delta(event.event.position.y, event.bounds)
                    } else {
                        None
                    };
                    this.line_drag_scroll.set(delta, cx, |delta, this, cx| {
                        if !cx.has_active_drag() || this.state.busy {
                            this.line_drag_scroll.stop();
                            return;
                        }
                        let offset = this.line_scroll.offset();
                        this.line_scroll.set_offset(point(
                            offset.x,
                            (offset.y - delta).clamp(-this.line_scroll.max_offset().y, px(0.)),
                        ));
                        cx.notify();
                    });
                }),
            )
            .p_1()
            .when(!self.state.snapshot.editor.lines.is_empty(), |el| {
                el.child(self.line_insert(0, cx))
            })
            .children(
                self.state
                    .snapshot
                    .editor
                    .lines
                    .iter()
                    .enumerate()
                    .flat_map(|(i, line)| [self.line(i, line, cx), self.line_insert(i + 1, cx)]),
            )
            .into_any_element()
    }

    fn line_append(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy
            || self.state.status.id.is_none()
            || self.state.snapshot.charas.is_empty();
        let chara = self.state.active_chara();
        div()
            .flex()
            .flex_shrink_0()
            .items_start()
            .gap_2()
            .px_1()
            .mt_2()
            .mb_4()
            .child(self.avatar(chara, 28.).mt(px(6.)))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .child(
                        icon_button("append-line", "plus")
                            .w_full()
                            .h(px(40.))
                            .rounded(px(8.))
                            .bg(p.control)
                            .border_2()
                            .border_color(p.line)
                            .disabled_with_cursor(disabled)
                            .on_click(cx.listener(|this, _, w, cx| {
                                let chara = this.state.snapshot.editor.active_chara.clone();
                                let inserted = if chara.is_empty() {
                                    this.state.add_line(false)
                                } else {
                                    this.state.append_chara_line(chara)
                                };
                                if inserted {
                                    this.edited(w, cx);
                                    this.line_scroll.scroll_to_bottom();
                                    this.fields.text.update(cx, |state, cx| state.focus(w, cx));
                                }
                            })),
                    )
                    .child(self.speech_tail(p.line)),
            )
            .into_any_element()
    }

    fn line_insert(&self, slot: usize, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let lines = &self.state.snapshot.editor.lines;
        let after = slot.checked_sub(1).map(|i| lines[i].id.clone());
        let drop_after = after.clone();
        let disabled = self.state.busy;
        let studio = cx.entity_id();
        div()
            .id(("lines-insert", slot))
            .group("lines-insert")
            .relative()
            .h(px(8.))
            .when(slot > 0 && slot < lines.len(), |el| {
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
                    .left_0()
                    .right_0()
                    .top(px(-4.))
                    .h(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(!disabled, |el| {
                        el.drag_over::<DraggedLine>(move |style, drag, _, _| {
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
                            move |this, drag: &DraggedLine, window, cx| {
                                if drag.studio != cx.entity_id() || this.state.busy {
                                    return;
                                }
                                if !this
                                    .state
                                    .snapshot
                                    .editor
                                    .lines
                                    .iter()
                                    .any(|line| line.id == drag.id)
                                {
                                    return;
                                }
                                this.select(drag.id.clone(), window, cx);
                                this.move_line(drop_after.as_deref(), cx);
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
                            .group_hover("lines-insert", |style| style.visible())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if this.state.insert_at(after.as_deref()) {
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

    fn line_scrollbar(&self) -> AnyElement {
        div()
            .relative()
            .w(px(24.))
            .flex_shrink_0()
            .child(
                Scrollbar::vertical(&self.line_scroll)
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
            .child(
                canvas(
                    |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
                    |_, hitbox, window, _| {
                        window.set_cursor_style(CursorStyle::PointingHand, &hitbox);
                    },
                )
                .absolute()
                .size_full(),
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
                .disabled_with_cursor(self.state.stopping)
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
