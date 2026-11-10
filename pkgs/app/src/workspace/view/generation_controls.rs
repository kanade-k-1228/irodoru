use super::Studio;
use gpui_kit::component::Sizable;
use gpui_kit::component::input::Input;
use gpui_kit::component::slider::{Slider, SliderState};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    fn generation_disabled(&self) -> bool {
        self.state.busy || self.state.status.id.is_none()
    }
    pub(super) fn speed_controls(&self) -> AnyElement {
        let form = &self.line_form;
        let settings = self.state.settings_value();
        let p = self.palette();
        let disabled = self.generation_disabled();
        div()
            .flex()
            .flex_col()
            .gap_0()
            .line_height(px(16.))
            .mx_3()
            .mb_3()
            .p_2()
            .rounded(px(8.))
            .border_1()
            .border_color(p.line)
            .bg(p.control)
            .text_xs()
            .text_color(p.muted)
            .child(
                div().flex().justify_between().mb_1().child("速度").child(
                    div()
                        .text_color(p.ink)
                        .border_b_1()
                        .border_color(p.line)
                        .px_1()
                        .child(format!("×{:.2}", settings.speed())),
                ),
            )
            .child(
                Slider::new(&form.speed)
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
        let form = &self.line_form;
        let p = self.palette();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .px_3()
            .pb_3()
            .text_xs()
            .text_color(p.muted)
            .child(self.generation_inputs())
            .children(
                [
                    (&form.text, "CFG 文"),
                    (&form.speaker, "CFG 声"),
                    (&form.caption, "CFG 説明"),
                ]
                .into_iter()
                .map(|(state, label)| self.guidance_slider(state, label, cx)),
            )
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
        let form = &self.line_form;
        let disabled = self.generation_disabled();
        div()
            .flex()
            .gap_3()
            .child(
                div().flex_1().min_w_0().child("シード").child(
                    Input::new(&form.seed)
                        .small()
                        .bg(p.control)
                        .disabled(disabled),
                ),
            )
            .child(
                div().flex_1().min_w_0().child("ステップ数").child(
                    Input::new(&form.steps)
                        .small()
                        .bg(p.control)
                        .disabled(disabled),
                ),
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
        let disabled = self.generation_disabled();
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
