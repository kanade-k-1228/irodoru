use crate::domain::ScriptRow;
use crate::workspace::view::Studio;
use crate::workspace::view::appearance::character_color;
use crate::workspace::view::script::DraggedRow;
use gpui_kit::component::Icon;
use gpui_kit::component::input::Textarea;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
mod actions;
mod editor;
mod takes;

impl Studio {
    pub(in crate::workspace::view) fn script_row(
        &self,
        i: usize,
        row: &ScriptRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
}
