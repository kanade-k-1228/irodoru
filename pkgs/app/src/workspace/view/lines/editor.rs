use crate::workspace::view::Studio;
use crate::workspace::view::appearance::DisabledWithCursor;
use crate::workspace::view::appearance::{button, icon, tool};
use gpui_kit::component::button::ButtonVariants;
use gpui_kit::*;

impl Studio {
    pub(in crate::workspace::view) fn line_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("line-editor")
            .flex()
            .items_stretch()
            .w_full()
            .min_w_0()
            .child(self.take_list(cx))
            .child(self.line_form(cx))
            .into_any_element()
    }

    fn line_form(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .w(relative(0.5))
            .border_l_1()
            .border_color(self.palette().muted.opacity(0.45))
            .min_w_0()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .child(self.line_prompt())
            .child(self.speed_controls(false))
            .child(self.advanced_controls(false, cx))
            .child(self.line_generation_actions(cx))
            .into_any_element()
    }

    fn line_generation_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.status.id.is_none();
        let generation_disabled =
            disabled || !self.can_generate() || self.fields.text.read(cx).value().trim().is_empty();
        div()
            .flex()
            .gap_2()
            .px_3()
            .pb_3()
            .child(
                button(
                    "generate-line",
                    if self.state.running.is_some() {
                        "生成中…"
                    } else {
                        "生成"
                    },
                )
                .primary()
                .bg(self.accent())
                .icon(icon("wand"))
                .flex_1()
                .disabled_with_cursor(generation_disabled)
                .on_click(cx.listener(|this, _, w, cx| this.generate(false, w, cx))),
            )
            .child(
                tool("advanced", "settings", "詳細設定")
                    .h(px(32.))
                    .w(px(34.))
                    .border_1()
                    .border_color(if self.line_form.expanded {
                        self.accent()
                    } else {
                        p.line
                    })
                    .bg(if self.line_form.expanded {
                        p.soft
                    } else {
                        p.control
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.line_form.expanded = !this.line_form.expanded;
                        cx.notify();
                    })),
            )
            .child(
                tool("random-line", "dice", "シードを変えて再生成")
                    .h(px(32.))
                    .w(px(34.))
                    .border_1()
                    .border_color(p.line)
                    .bg(p.control)
                    .disabled_with_cursor(disabled || !self.can_generate())
                    .on_click(cx.listener(|this, _, w, cx| this.generate(true, w, cx))),
            )
            .into_any_element()
    }
}
