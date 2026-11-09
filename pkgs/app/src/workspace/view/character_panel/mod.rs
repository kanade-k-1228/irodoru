use crate::workspace::view::Studio;
use crate::workspace::view::appearance::{button, icon};
use gpui_kit::component::Disableable;
use gpui_kit::*;
mod editor;
mod list;

impl Studio {
    pub(in crate::workspace::view) fn character_panel(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let narrow = window.viewport_size().width < px(640.);
        div()
            .flex()
            .h(px(if self.state.draft.is_some() {
                (f32::from(window.viewport_size().height) * 0.5).min(360.)
            } else {
                136.
            }))
            .flex_shrink_0()
            .min_h_0()
            .border_t_1()
            .border_color(p.line)
            .bg(p.panel)
            .child(self.characters(narrow, cx))
            .child(if self.state.draft.is_some() {
                self.character_editor(narrow, cx)
            } else {
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        button("create-character", "キャラクターを作成")
                            .icon(icon("plus"))
                            .disabled(self.state.busy || self.state.status.id.is_none())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.edit_character(None, window, cx);
                            })),
                    )
                    .into_any_element()
            })
            .into_any_element()
    }
}
