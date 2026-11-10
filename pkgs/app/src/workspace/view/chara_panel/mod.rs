use crate::workspace::view::Studio;
use crate::workspace::view::appearance::{button, icon};
use gpui_kit::component::Disableable;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
mod editor;
mod list;

impl Studio {
    pub(in crate::workspace::view) fn chara_panel(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        div()
            .flex()
            .h(px(if self.state.active_chara().is_some() {
                (f32::from(window.viewport_size().height) * 0.5).min(360.)
            } else {
                136.
            }))
            .flex_shrink_0()
            .min_h_0()
            .border_t_1()
            .border_color(p.line)
            .bg(p.panel)
            .child(self.charas(cx))
            .child(if self.state.active_chara().is_some() {
                self.chara_editor(cx)
            } else {
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(self.state.snapshot.charas.is_empty(), |el| {
                        el.child(
                            button("create-chara", "キャラクターを作成")
                                .icon(icon("plus"))
                                .disabled(self.state.busy || self.state.status.id.is_none())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.create_chara(cx);
                                })),
                        )
                    })
                    .when(!self.state.snapshot.charas.is_empty(), |el| {
                        el.child(
                            div()
                                .px_4()
                                .text_sm()
                                .text_color(p.muted)
                                .child("左のキャラクターを選択してください。"),
                        )
                    })
                    .into_any_element()
            })
            .into_any_element()
    }
}
