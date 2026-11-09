//! Script list composition; rows and insertion gaps are sibling elements.
use super::Studio;
use super::appearance::{button, character_color, tool};
use crate::domain::{AudioSource, ScriptRow};
use gpui_kit::component::input::Textarea;
use gpui_kit::component::scroll::{AutoScroll, Scrollbar, ScrollbarMode};
use gpui_kit::component::{Disableable, Icon};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

#[derive(Clone)]
struct DraggedRow {
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

impl Studio {
    pub(super) fn script(&self, cx: &mut Context<Self>) -> AnyElement {
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
        let disabled = self.state.busy || self.state.draft.is_some();
        let drag = DraggedRow {
            studio: cx.entity_id(),
            id: row.id.clone(),
            text: if row.text.is_empty() {
                "セリフを移動".into()
            } else {
                row.text.clone()
            },
        };
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
                            .id(("row-avatar", i))
                            .when(!disabled, |el| {
                                el.cursor_move()
                                    .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
                            })
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

    fn script_row_actions(&self, i: usize, row: &ScriptRow, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.state.busy || self.state.draft.is_some();
        div()
            .flex()
            .children(
                [
                    (0, "plus", "上に追加"),
                    (2, "copy", "複製"),
                    (3, "trash", "削除"),
                ]
                .into_iter()
                .map(|(action, icon, label)| {
                    let id = row.id.clone();
                    let disabled =
                        disabled || (action == 3 && self.state.snapshot.editor.rows.len() <= 1);
                    tool(("row-action", i * 4 + action), icon, label)
                        .size(px(40.))
                        .rounded_none()
                        .disabled(disabled)
                        .when(disabled, |el| el.cursor_default())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this.state.busy || this.state.draft.is_some() {
                                return;
                            }
                            let Some(index) = this
                                .state
                                .snapshot
                                .editor
                                .rows
                                .iter()
                                .position(|row| row.id == id)
                            else {
                                return;
                            };
                            let after = if action == 0 {
                                index
                                    .checked_sub(1)
                                    .map(|index| this.state.snapshot.editor.rows[index].id.clone())
                            } else {
                                Some(id.clone())
                            };
                            this.state.select(id.clone());
                            let changed = if action == 3 {
                                this.state.remove_row()
                            } else {
                                this.state.insert_at(after.as_deref(), action == 2)
                            };
                            if changed {
                                this.edited(window, cx);
                                this.fields
                                    .text
                                    .update(cx, |state, cx| state.focus(window, cx));
                            }
                        }))
                }),
            )
            .into_any_element()
    }

    fn script_play_button(&self, i: usize, row: &ScriptRow, cx: &mut Context<Self>) -> AnyElement {
        let source = row
            .take_id
            .as_ref()
            .filter(|id| {
                self.state
                    .snapshot
                    .takes
                    .iter()
                    .any(|take| &take.id == *id && take.row_id == row.id)
            })
            .map(|id| AudioSource::Take(id.clone()));
        let playing = source.is_some()
            && self.audio_id == source
            && self
                .playback
                .as_ref()
                .is_some_and(|player| player.playing());
        let disabled = self.state.busy || source.is_none();
        tool(
            ("row-play", i),
            if playing { "pause" } else { "play" },
            if source.is_none() {
                "音声を生成すると再生できます"
            } else {
                "再生 / 一時停止"
            },
        )
        .size(px(40.))
        .rounded_none()
        .disabled(disabled)
        .when(disabled, |el| el.cursor_default())
        .on_click(cx.listener(move |this, _, _, cx| {
            if let Some(source) = &source {
                this.play(source.clone());
                cx.notify();
            }
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
        let speech = div()
            .id(("speech", i))
            .relative()
            .line_height(px(24.))
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h(px(40.))
            .rounded(px(8.))
            .border_2()
            .border_color(if selected { color } else { p.line })
            .bg(p.control)
            .text_sm()
            .child(
                div()
                    .flex()
                    .items_stretch()
                    .min_w_0()
                    .child(
                        div()
                            .w(px(40.))
                            .flex_shrink_0()
                            .child(self.script_play_button(i, row, cx)),
                    )
                    .child(
                        div()
                            .id(("speech-content", i))
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .child(if selected {
                                Textarea::new(&self.fields.text)
                                    .appearance(false)
                                    .bordered(false)
                                    .w_full()
                                    .text_sm()
                                    .line_height(px(24.))
                                    .disabled(
                                        self.state.busy
                                            || self.state.draft.is_some()
                                            || self.state.status.id.is_none(),
                                    )
                                    .into_any_element()
                            } else {
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .px_2()
                                    .py_1()
                                    .text_color(if row.text.is_empty() { p.muted } else { p.ink })
                                    .child(if row.text.is_empty() {
                                        "セリフを入力".to_owned()
                                    } else {
                                        row.text.clone()
                                    })
                                    .into_any_element()
                            })
                            .when(!selected, |el| {
                                el.cursor_text()
                                    .hover(|s| s.bg(p.hover))
                                    .on_click(cx.listener(move |this, _, w, cx| {
                                        if this.state.busy || this.state.draft.is_some() {
                                            return;
                                        }
                                        this.select(select.clone(), w, cx);
                                        this.fields.text.update(cx, |state, cx| state.focus(w, cx));
                                    }))
                            }),
                    )
                    .child(
                        div()
                            .w(px(121.))
                            .flex_shrink_0()
                            .border_l_1()
                            .border_color(p.line)
                            .child(self.script_row_actions(i, row, cx)),
                    ),
            )
            .when(selected && self.state.draft.is_none(), |el| {
                el.child(
                    div()
                        .border_t_1()
                        .border_color(p.muted.opacity(0.45))
                        .child(self.line_editor(cx)),
                )
            })
            .into_any_element();
        div()
            .relative()
            .flex_1()
            .min_w_0()
            .child(speech)
            .child(
                div()
                    .absolute()
                    .left(px(-8.))
                    .top(px(10.))
                    .size(px(16.))
                    .child(
                        Icon::default()
                            .data(b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'><path fill='currentColor' d='M16 1H10L2 8L10 15H16Z'/></svg>" as &'static [u8])
                            .size(px(16.))
                            .text_color(p.control),
                    )
                    .child(
                        Icon::default()
                            .data(b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'><path fill='none' stroke='currentColor' stroke-width='2' stroke-linejoin='round' d='M10 1L2 8L10 15'/></svg>" as &'static [u8])
                            .absolute()
                            .top_0()
                            .left_0()
                            .size(px(16.))
                            .text_color(if selected { color } else { p.line }),
                    ),
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
