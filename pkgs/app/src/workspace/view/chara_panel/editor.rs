//! Chara editor composition, followed by each form section.
use crate::domain::Chara;
use crate::workspace::view::Studio;
use crate::workspace::view::appearance::DisabledWithCursor;
use crate::workspace::view::appearance::{button, icon, tool};
use gpui_kit::component::Disableable;
use gpui_kit::component::color_picker::ColorPicker;
use gpui_kit::component::input::{Input, Textarea};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

const CHARA_ICON_SIZE: f32 = 48.;

impl Studio {
    pub(in crate::workspace::view) fn chara_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let chara = self.state.active_chara().unwrap();
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .child(self.chara_form_wheel.probe())
            .child(self.chara_form(chara, cx))
            .into_any_element()
    }

    fn chara_form(&self, chara: &Chara, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("chara-form")
            .flex_1()
            .min_h_0()
            .p_3()
            .flex()
            .gap_4()
            .child(self.chara_tools(chara, cx))
            .child(self.chara_details())
            .child(self.chara_references(chara, cx))
            .into_any_element()
    }

    fn chara_details(&self) -> AnyElement {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(self.chara_color_picker())
                    .child(
                        Input::new(&self.fields.name)
                            .flex_1()
                            .min_w_0()
                            .bg(self.palette().control)
                            .disabled(self.state.busy || self.state.chara_locked),
                    ),
            )
            .child(
                Textarea::new(&self.fields.description)
                    .flex_1()
                    .min_h_0()
                    .bg(self.palette().control)
                    .disabled(self.state.busy || self.state.chara_locked),
            )
            .into_any_element()
    }

    fn chara_color_picker(&self) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.chara_locked;
        let color_swatch = icon("color")
            .size(px(32.))
            .rounded(px(8.))
            .border_1()
            .border_color(p.line)
            .bg(p.control)
            .text_color(self.accent());
        div()
            .size(px(32.))
            .flex_shrink_0()
            .child(if disabled {
                color_swatch.opacity(0.5).into_any_element()
            } else {
                ColorPicker::new(&self.fields.color)
                    .cursor_pointer()
                    .icon(color_swatch)
                    .size(px(32.))
                    .accessibility_label("テーマカラー")
                    .into_any_element()
            })
            .into_any_element()
    }

    fn chara_tools(&self, chara: &Chara, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.chara_locked;
        div()
            .w(px(CHARA_ICON_SIZE))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .child(
                div()
                    .relative()
                    .size(px(CHARA_ICON_SIZE))
                    .flex_shrink_0()
                    .child(self.chara_icon_button(chara, cx))
                    .when(chara.icon.as_ref().is_some_and(|i| !i.is_empty()), |el| {
                        el.child(
                            div().absolute().top_0().right_0().child(
                                tool("clear-icon", "close", "画像を削除")
                                    .size(px(24.))
                                    .bg(p.panel)
                                    .rounded_full()
                                    .disabled_with_cursor(disabled)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.state.set_chara_icon(String::new());
                                        this.autosave_chara();
                                        cx.notify();
                                    })),
                            ),
                        )
                    }),
            )
            .child(
                tool(
                    "lock-chara",
                    if self.state.chara_locked {
                        "lock"
                    } else {
                        "unlock"
                    },
                    if self.state.chara_locked {
                        "ロック解除"
                    } else {
                        "パラメータをロック"
                    },
                )
                .when(self.state.chara_locked, |el| {
                    el.bg(p.soft).text_color(self.accent())
                })
                .disabled(self.state.busy)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.state.chara_locked = !this.state.chara_locked;
                    this.sync_fields(window, cx);
                    cx.notify();
                })),
            )
            .child(
                tool("export-chara", "download", "キャラクターファイルを書き出す")
                    .disabled(self.state.busy)
                    .on_click(cx.listener(|this, _, w, cx| this.chara_file(true, w, cx))),
            )
            .map(|el| {
                let id = chara.id.clone();
                let name = chara.name.clone();
                let in_use = self.state.chara_in_use(&id);
                el.child(
                    tool(
                        "delete-chara",
                        "trash",
                        if in_use {
                            "セリフで使用中のキャラクターは削除できません"
                        } else {
                            "キャラを削除"
                        },
                    )
                    .bg(rgb(0xb52e42))
                    .text_color(rgb(0xffffff))
                    .disabled_with_cursor(disabled || in_use)
                    .when(disabled || in_use, |el| {
                        el.bg(p.control).text_color(p.muted)
                    })
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.delete_chara(id.clone(), name.clone(), w, cx)
                    })),
                )
            })
            .into_any_element()
    }

    fn chara_icon_button(&self, chara: &Chara, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.state.busy || self.state.chara_locked;
        button("import-icon", "")
            .size(px(CHARA_ICON_SIZE))
            .flex_shrink_0()
            .rounded_full()
            .overflow_hidden()
            .when(chara.icon.as_ref().is_none_or(|i| i.is_empty()), |el| {
                el.child(
                    div()
                        .text_xl()
                        .child(chara.name.chars().next().unwrap_or('？').to_string()),
                )
            })
            .when_some(self.chara_icon(Some(chara)), |el, image| {
                el.child(img(image).size(px(CHARA_ICON_SIZE - 4.)).rounded_full())
            })
            .disabled_with_cursor(disabled)
            .tooltip("画像を選択")
            .on_click(cx.listener(|this, _, w, cx| this.import(true, w, cx)))
            .into_any_element()
    }
}
