use super::appearance::{blend, button, character_color, icon, tool};
use super::{DragRow, Studio};
use crate::workspace::editor;
use gpui_kit::component::Disableable;
use gpui_kit::component::scroll::{Scrollbar, ScrollbarMode};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(super) fn script(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.draft.is_some();
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
                    .child(
                        div()
                            .id("script")
                            .flex_1()
                            .min_w_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.script_scroll)
                            .p_1()
                            .children(self.state.snapshot.editor.rows.iter().enumerate().map(
                                |(i, row)| {
                                    let id = row.id.clone();
                                    let select = id.clone();
                                    let target = id.clone();
                                    let insert = id.clone();
                                    let selected = self.state.selected.as_ref() == Some(&id);
                                    let running = self.state.running.as_ref() == Some(&id);
                                    let character = self
                                        .state
                                        .snapshot
                                        .characters
                                        .iter()
                                        .find(|c| c.id == row.character_id);
                                    let last = i + 1 == self.state.snapshot.editor.rows.len();
                                    let color = character_color(
                                        character.map(|c| c.color.as_str()),
                                        p.accent,
                                    );
                                    div()
                                        .id(("row", i))
                                        .relative()
                                        .rounded(px(8.))
                                        .px_1()
                                        .py_1()
                                        .when(!last, |el| el.pb(px(24.)))
                                        .when(selected, |el| el.bg(blend(color, p.panel, 0.06)))
                                        .when(running, |el| el.border_1().border_color(p.accent))
                                        .child(
                                            div()
                                                .flex()
                                                .items_start()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .id(("drag", i))
                                                        .w(px(16.))
                                                        .h(px(28.))
                                                        .flex_shrink_0()
                                                        .flex()
                                                        .items_center()
                                                        .text_color(p.muted.opacity(0.4))
                                                        .cursor_move()
                                                        .child(icon("grip"))
                                                        .when(!disabled, |el| {
                                                            el.on_drag(
                                                                DragRow(id.clone()),
                                                                |drag, _, _, cx| {
                                                                    cx.new(|_| drag.clone())
                                                                },
                                                            )
                                                        }),
                                                )
                                                .child(
                                                    div()
                                                        .relative()
                                                        .w(px(28.))
                                                        .flex_shrink_0()
                                                        .child(
                                                            div()
                                                                .absolute()
                                                                .left(px(14.))
                                                                .top(px(15.))
                                                                .w(px(1.))
                                                                .h(px(if last { 0. } else { 42. }))
                                                                .bg(p.line),
                                                        )
                                                        .child(self.avatar(character, 28.)),
                                                )
                                                .child(
                                                    div()
                                                        .id(("speech", i))
                                                        .line_height(px(24.))
                                                        .relative()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .min_h(px(28.))
                                                        .rounded(px(8.))
                                                        .border_1()
                                                        .border_color(if selected {
                                                            p.surface
                                                        } else {
                                                            p.line
                                                        })
                                                        .bg(if selected {
                                                            blend(p.control, rgb(0).into(), 0.78)
                                                        } else {
                                                            p.control
                                                        })
                                                        .px_2()
                                                        .py(px(2.))
                                                        .pr(if selected { px(36.) } else { px(8.) })
                                                        .text_sm()
                                                        .text_color(if selected {
                                                            p.ink
                                                        } else if row.text.is_empty() {
                                                            p.muted
                                                        } else {
                                                            p.ink
                                                        })
                                                        .cursor_pointer()
                                                        .hover(|s| s.bg(p.hover))
                                                        .child(if row.text.is_empty() {
                                                            "セリフを入力".to_owned()
                                                        } else {
                                                            row.text.clone()
                                                        })
                                                        .on_click(cx.listener(
                                                            move |this, _, w, cx| {
                                                                this.select(select.clone(), w, cx);
                                                                this.fields
                                                                    .text
                                                                    .update(cx, |state, cx| {
                                                                        state.focus(w, cx)
                                                                    });
                                                            },
                                                        )),
                                                ),
                                        )
                                        .when(selected, |el| {
                                            el.child(
                                                div().absolute().right(px(8.)).top(px(6.)).child(
                                                    tool(("remove", i), "trash", "選択行を削除")
                                                        .bg(rgb(0xb52e42))
                                                        .text_color(rgb(0xffffff))
                                                        .disabled(
                                                            disabled
                                                                || self
                                                                    .state
                                                                    .snapshot
                                                                    .editor
                                                                    .rows
                                                                    .len()
                                                                    <= 1,
                                                        )
                                                        .on_click(cx.listener(|this, _, w, cx| {
                                                            this.remove_row(w, cx)
                                                        })),
                                                ),
                                            )
                                        })
                                        .when(!last, |el| {
                                            el.child(
                                                div()
                                                    .absolute()
                                                    .left(px(32.))
                                                    .bottom(px(2.))
                                                    .child(
                                                        tool(
                                                            ("insert", i),
                                                            "plus",
                                                            "ここにセリフを追加",
                                                        )
                                                        .size(px(20.))
                                                        .rounded_full()
                                                        .bg(p.panel)
                                                        .border_1()
                                                        .border_color(p.line)
                                                        .disabled(
                                                            disabled
                                                                || self
                                                                    .state
                                                                    .snapshot
                                                                    .editor
                                                                    .rows
                                                                    .len()
                                                                    >= editor::MAX_ROWS,
                                                        )
                                                        .on_click(cx.listener(
                                                            move |this, _, w, cx| {
                                                                this.state.selected =
                                                                    Some(insert.clone());
                                                                this.add(false, w, cx);
                                                            },
                                                        )),
                                                    ),
                                            )
                                        })
                                        .when_some(
                                            self.state.row_errors.get(&id).cloned(),
                                            |el, error| {
                                                el.child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(p.warning)
                                                        .child(error),
                                                )
                                            },
                                        )
                                        .on_drop(cx.listener(move |this, drag: &DragRow, w, cx| {
                                            if !this.state.busy && this.state.draft.is_none() {
                                                editor::reorder(
                                                    &mut this.state.snapshot.editor.rows,
                                                    &drag.0,
                                                    &target,
                                                    false,
                                                );
                                                this.select(drag.0.clone(), w, cx);
                                            }
                                        }))
                                },
                            )),
                    )
                    .child(
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
                            .child(div().relative().flex_1().w(px(24.)).child(
                                if self.script_scroll.max_offset().y <= px(0.) {
                                    div()
                                        .w(px(4.))
                                        .h_full()
                                        .mx_auto()
                                        .rounded_full()
                                        .child(
                                            div()
                                                .w(px(12.))
                                                .h(px(40.))
                                                .ml(px(-4.))
                                                .rounded(px(6.))
                                                .bg(if self.settings.light {
                                                    rgb(0x9ba8b8)
                                                } else {
                                                    rgb(0x397cae)
                                                }),
                                        )
                                        .into_any_element()
                                } else {
                                    Scrollbar::vertical(&self.script_scroll)
                                        .viewport_from_layout()
                                        .mode(ScrollbarMode::Always)
                                        .styles(|s| {
                                            s.track(|s| s.bg(transparent_black()).width(px(4.)))
                                                .thumb(|s| {
                                                    s.bg(if self.settings.light {
                                                        rgb(0x9ba8b8)
                                                    } else {
                                                        rgb(0x397cae)
                                                    })
                                                    .width(px(12.))
                                                })
                                        })
                                        .into_any_element()
                                },
                            ))
                            .child(
                                tool("scroll-bottom", "down", "台本の末尾へ")
                                    .size(px(24.))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.script_scroll.scroll_to_bottom();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .when(self.state.running.is_some(), |el| {
                el.child(
                    div().p_1().child(
                        button(
                            "stop",
                            if self.state.stopping {
                                "停止待ち"
                            } else {
                                "停止"
                            },
                        )
                        .disabled(self.state.stopping)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(s) = &this.service {
                                s.stop();
                                this.state.stopping = true;
                            }
                            cx.notify();
                        })),
                    ),
                )
            })
            .into_any_element()
    }
}
