use super::Studio;
use super::appearance::{button, tool};
use crate::workspace::editor;
use gpui_kit::component::Disableable;
use gpui_kit::component::Sizable;
use gpui_kit::component::input::Input;
use gpui_kit::component::slider::Slider;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(super) fn controls(&self, _cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.status.id.is_none();
        div()
            .flex()
            .flex_col()
            .gap_0()
            .line_height(px(16.))
            .p_3()
            .pt(px(6.))
            .text_xs()
            .text_color(p.muted)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .mb_1()
                    .child("長さ（倍率）")
                    .child(
                        div()
                            .text_color(p.ink)
                            .border_b_1()
                            .border_color(p.line)
                            .px_1()
                            .child(format!(
                                "{:.2}×",
                                self.state.settings_value().duration_scale
                            )),
                    ),
            )
            .child(
                Slider::new(&self.sliders[0])
                    .bg(self.accent())
                    .text_color(self.accent())
                    .h(px(16.))
                    .flex_none()
                    .disabled(disabled),
            )
            .child(div().flex().justify_between().child("速く").child("遅く"))
            .into_any_element()
    }
    pub(super) fn advanced_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.status.id.is_none();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .px_3()
            .pb_3()
            .text_xs()
            .text_color(p.muted)
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div().flex_1().min_w_0().child("シード").child(
                            Input::new(&self.fields.seed)
                                .small()
                                .bg(p.control)
                                .disabled(disabled),
                        ),
                    )
                    .child(
                        div().flex_1().min_w_0().child("ステップ数").child(
                            Input::new(&self.fields.steps)
                                .small()
                                .bg(p.control)
                                .disabled(disabled),
                        ),
                    ),
            )
            .children(
                self.sliders
                    .iter()
                    .skip(1)
                    .zip(["CFG 文", "CFG 声", "CFG 説明"])
                    .map(|(state, label)| {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div().flex().justify_between().child(label).child(
                                    div()
                                        .text_color(p.ink)
                                        .child(state.read(cx).value().to_string()),
                                ),
                            )
                            .child(
                                Slider::new(state)
                                    .bg(self.accent())
                                    .text_color(self.accent())
                                    .disabled(disabled),
                            )
                    }),
            )
            .when(self.state.draft.is_none(), |el| {
                el.child(
                    Input::new(&self.fields.prompt)
                        .small()
                        .bg(p.control)
                        .disabled(disabled),
                )
            })
            .when(self.state.draft.is_none(), |el| {
                el.child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            button("reset-settings", "既定設定に戻す")
                                .disabled(disabled)
                                .on_click(cx.listener(|this, _, w, cx| {
                                    if let Some(r) = this.state.selected_row_mut() {
                                        r.settings = None;
                                    }
                                    this.autosave();
                                    this.sync_fields(w, cx);
                                    cx.notify();
                                })),
                        )
                        .child(
                            tool("duplicate-line", "copy", "行を複製")
                                .disabled(
                                    disabled
                                        || self.state.snapshot.editor.rows.len()
                                            >= editor::MAX_ROWS,
                                )
                                .on_click(cx.listener(|this, _, w, cx| this.add(true, w, cx))),
                        ),
                )
            })
            .when(self.state.draft.is_none(), |el| {
                el.child(
                    button("generate-all", "すべて生成")
                        .disabled(disabled || !self.can_generate())
                        .on_click(cx.listener(|this, _, w, cx| this.generate(true, false, w, cx))),
                )
            })
            .into_any_element()
    }
}
