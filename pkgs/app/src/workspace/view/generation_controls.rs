use super::Studio;
use super::appearance::{button, tool};
use gpui_kit::component::Disableable;
use gpui_kit::component::Sizable;
use gpui_kit::component::input::Input;
use gpui_kit::component::slider::{Slider, SliderState};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(super) fn speed_controls(&self) -> AnyElement {
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
                div().flex().justify_between().mb_1().child("速度").child(
                    div()
                        .text_color(p.ink)
                        .border_b_1()
                        .border_color(p.line)
                        .px_1()
                        .child(format!("×{:.2}", self.state.settings_value().speed())),
                ),
            )
            .child(
                Slider::new(&self.sliders.speed)
                    .when(!disabled, |el| el.cursor_pointer())
                    .bg(self.accent())
                    .text_color(self.accent())
                    .h(px(16.))
                    .flex_none()
                    .disabled(disabled),
            )
            .child(
                div()
                    .flex()
                    .child(div().flex_1().child("×0.5（÷2）"))
                    .child(div().flex_1().text_center().child("×1.0"))
                    .child(div().flex_1().text_right().child("×2.0")),
            )
            .into_any_element()
    }
    pub(super) fn advanced_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .px_3()
            .pb_3()
            .text_xs()
            .text_color(p.muted)
            .when(self.advanced, |el| {
                el.child(self.generation_inputs()).children(
                    [
                        (&self.sliders.text, "CFG 文"),
                        (&self.sliders.speaker, "CFG 声"),
                        (&self.sliders.caption, "CFG 説明"),
                    ]
                    .into_iter()
                    .map(|(state, label)| self.guidance_slider(state, label, cx)),
                )
            })
            .when(self.state.draft.is_none(), |el| {
                el.child(self.line_settings_actions(cx))
            })
            .into_any_element()
    }

    pub(super) fn line_prompt(&self) -> AnyElement {
        div()
            .px_3()
            .pt_3()
            .pb_2()
            .child(
                Input::new(&self.fields.prompt)
                    .small()
                    .bg(self.palette().control)
                    .disabled(self.state.busy || self.state.status.id.is_none()),
            )
            .into_any_element()
    }

    fn generation_inputs(&self) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.status.id.is_none();
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
            )
            .into_any_element()
    }

    fn line_settings_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.state.busy || self.state.status.id.is_none();
        div()
            .flex()
            .gap_2()
            .child(
                button("reset-settings", "既定設定に戻す")
                    .disabled(disabled)
                    .when(disabled, |el| el.cursor_default())
                    .on_click(cx.listener(|this, _, w, cx| {
                        if this.state.reset_row_settings() {
                            this.edited(w, cx);
                        }
                    })),
            )
            .child(
                tool("duplicate-line", "copy", "行を複製")
                    .disabled(disabled)
                    .when(disabled, |el| el.cursor_default())
                    .on_click(cx.listener(|this, _, w, cx| this.add(true, w, cx))),
            )
            .into_any_element()
    }

    fn guidance_slider(
        &self,
        state: &Entity<SliderState>,
        label: &'static str,
        cx: &Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.status.id.is_none();
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
                    .when(!disabled, |el| el.cursor_pointer())
                    .bg(self.accent())
                    .text_color(self.accent())
                    .disabled(disabled),
            )
            .into_any_element()
    }
}
