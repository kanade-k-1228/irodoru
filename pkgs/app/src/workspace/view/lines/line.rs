use crate::domain::Line;
use crate::workspace::view::Studio;
use crate::workspace::view::appearance::chara_color;
use crate::workspace::view::components::speech_row;
use crate::workspace::view::lines::DraggedLine;
use gpui_kit::component::input::Textarea;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(in crate::workspace::view) fn line(
        &self,
        i: usize,
        line: &Line,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let id = line.id.clone();
        let selected = self.state.selected.as_ref() == Some(&id);
        let running = self.state.running.as_ref() == Some(&id);
        let chara = self
            .state
            .snapshot
            .charas
            .iter()
            .find(|c| c.id == line.chara_id);
        let color = chara_color(chara.map(|c| c.color.as_str()), p.accent);
        let disabled = self.state.busy;
        let drag = DraggedLine {
            studio: cx.entity_id(),
            id: line.id.clone(),
            text: if line.text.is_empty() {
                "セリフを移動".into()
            } else {
                line.text.clone()
            },
        };
        let last = i + 1 == self.state.snapshot.editor.lines.len();
        div()
            .id(("line", i))
            .relative()
            .rounded(px(8.))
            .px_1()
            .py_1()
            .when(running, |el| el.border_1().border_color(p.accent))
            // Continue the rail through tall lines, behind the opaque avatars.
            .when(self.state.snapshot.editor.lines.len() > 1, |el| {
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
            .child(speech_row(
                self.avatar(chara, 28.)
                    .id(("line-avatar", i))
                    .when(!disabled, |el| {
                        el.cursor_move()
                            .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
                    })
                    .when(selected, |el| el.border_2().border_color(color)),
                self.line_speech(i, line, selected, color, cx),
            ))
            .when_some(self.state.line_errors.get(&id).cloned(), |el, error| {
                el.child(div().text_xs().text_color(p.warning).child(error))
            })
            .into_any_element()
    }

    fn line_speech(
        &self,
        i: usize,
        line: &Line,
        selected: bool,
        color: Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let select = line.id.clone();
        let speech = div()
            .id(("speech", i))
            .relative()
            .line_height(px(24.))
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
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
                            .child(self.line_play_button(i, line, cx)),
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
                                    .disabled(self.state.busy || self.state.status.id.is_none())
                                    .into_any_element()
                            } else {
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .px_2()
                                    .py_1()
                                    .text_color(if line.text.is_empty() { p.muted } else { p.ink })
                                    .child(if line.text.is_empty() {
                                        "セリフを入力".to_owned()
                                    } else {
                                        line.text.clone()
                                    })
                                    .into_any_element()
                            })
                            .when(!selected, |el| {
                                el.cursor_text()
                                    .hover(|s| s.bg(p.hover))
                                    .on_click(cx.listener(move |this, _, w, cx| {
                                        if this.state.busy {
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
                            .child(self.line_actions(i, line, cx)),
                    ),
            )
            .when(self.state.expanded_line.as_ref() == Some(&line.id), |el| {
                el.child(
                    div()
                        .border_t_1()
                        .border_color(p.muted.opacity(0.45))
                        .child(self.line_editor(cx)),
                )
            });
        self.speech_bubble(if selected { color } else { p.line }, speech)
            .into_any_element()
    }
}
