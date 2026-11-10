//! Chara editor composition, followed by each form section.
use crate::domain::AudioSource;
use crate::domain::Chara;
use crate::workspace::view::Studio;
use crate::workspace::view::appearance::DisabledWithCursor;
use crate::workspace::view::appearance::{button, icon, tool};
use gpui_kit::component::Disableable;
use gpui_kit::component::Sizable;
use gpui_kit::component::button::Button;
use gpui_kit::component::button::ButtonVariants;
use gpui_kit::component::color_picker::ColorPicker;
use gpui_kit::component::input::{Input, Textarea};
use gpui_kit::component::slider::Slider;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(in crate::workspace::view) fn chara_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let chara = self.state.active_chara().unwrap();
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .child(self.chara_form(chara, cx))
            .into_any_element()
    }

    fn chara_form(&self, chara: &Chara, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("chara-form")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_4()
            .flex()
            .gap_4()
            .child(self.chara_details(chara, cx))
            .child(self.chara_preview(chara, cx))
            .into_any_element()
    }

    fn chara_details(&self, chara: &Chara, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.chara_locked;
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.chara_identity(chara, cx))
            .child(self.chara_description())
            .child(self.reference_voice_actions(cx))
            .child(div().text_xs().text_color(p.muted).child(format!(
                "参照音声 {}件 — 選択したすべての音声を使用",
                chara.voice_ids.len()
            )))
            .children(self.state.snapshot.voices.iter().enumerate().map(|(i, v)| {
                let voice = v.id().to_owned();
                let selected = chara.voice_ids.contains(&voice);
                Button::new(("voice", i))
                    .cursor_pointer()
                    .small()
                    .label(format!(
                        "{} {}",
                        if selected { "✓" } else { "＋" },
                        v.label()
                    ))
                    .when(selected, |el| el.bg(p.soft).border_color(self.accent()))
                    .disabled_with_cursor(disabled)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.state.toggle_chara_voice(Some(voice.clone()));
                        this.autosave_chara();
                        cx.notify();
                    }))
            }))
            .into_any_element()
    }

    fn chara_identity(&self, chara: &Chara, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.chara_locked;
        div()
            .flex()
            .items_end()
            .gap_2()
            .child(self.chara_icon_button(chara, cx))
            .when(chara.icon.as_ref().is_some_and(|i| !i.is_empty()), |el| {
                el.child(
                    tool("clear-icon", "close", "画像を削除")
                        .disabled_with_cursor(disabled)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.state.set_chara_icon(String::new());
                            this.autosave_chara();
                            cx.notify();
                        })),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        Input::new(&self.fields.name)
                            .bg(p.control)
                            .disabled(disabled),
                    )
                    .child(self.chara_identity_actions(chara, cx)),
            )
            .into_any_element()
    }

    fn chara_identity_actions(&self, chara: &Chara, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.chara_locked;
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .w(px(32.))
                    .h(px(32.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(p.muted)
                    .child(if disabled {
                        div()
                            .size(px(32.))
                            .rounded(px(8.))
                            .bg(self.accent())
                            .opacity(0.5)
                            .into_any_element()
                    } else {
                        ColorPicker::new(&self.fields.color)
                            .cursor_pointer()
                            .with_size(px(32.))
                            .size(px(32.))
                            .accessibility_label("テーマカラー")
                            .into_any_element()
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
                .size(px(32.))
                .disabled(self.state.busy)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.state.chara_locked = !this.state.chara_locked;
                    this.sync_fields(window, cx);
                    cx.notify();
                })),
            )
            .map(|el| {
                let id = chara.id.clone();
                let name = chara.name.clone();
                el.child(
                    tool("delete-chara", "trash", "キャラを削除")
                        .size(px(32.))
                        .bg(rgb(0xb52e42))
                        .text_color(rgb(0xffffff))
                        .disabled(disabled || self.state.chara_locked)
                        .when(disabled || self.state.chara_locked, |el| {
                            el.cursor_default()
                        })
                        .on_click(cx.listener(move |this, _, w, cx| {
                            this.delete_chara(id.clone(), name.clone(), w, cx)
                        })),
                )
            })
            .child(
                tool("export-chara", "download", "キャラクターファイルを書き出す")
                    .size(px(32.))
                    .disabled(self.state.busy)
                    .on_click(cx.listener(|this, _, w, cx| this.chara_file(true, w, cx))),
            )
            .into_any_element()
    }

    fn chara_icon_button(&self, chara: &Chara, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.state.busy || self.state.chara_locked;
        button("import-icon", "")
            .size(px(64.))
            .flex_shrink_0()
            .rounded_full()
            .overflow_hidden()
            .when(chara.icon.as_ref().is_none_or(|i| i.is_empty()), |el| {
                el.child(
                    div()
                        .text_2xl()
                        .child(chara.name.chars().next().unwrap_or('？').to_string()),
                )
            })
            .when_some(
                crate::workspace::view::components::chara_icon(chara.icon.as_deref()),
                |el, image| el.child(img(image).size(px(60.)).rounded_full()),
            )
            .disabled_with_cursor(disabled)
            .tooltip("画像を選択")
            .on_click(cx.listener(|this, _, w, cx| this.import(true, w, cx)))
            .into_any_element()
    }

    fn chara_description(&self) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.chara_locked;
        div()
            .text_xs()
            .text_color(p.muted)
            .child(
                Textarea::new(&self.fields.description)
                    .h(px(112.))
                    .bg(p.control)
                    .disabled(disabled),
            )
            .into_any_element()
    }

    fn reference_voice_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.chara_locked;
        div()
            .flex()
            .items_center()
            .gap_2()
            .text_xs()
            .text_color(p.muted)
            .child(
                tool("import-voice", "folder", "音声を取り込む")
                    .disabled_with_cursor(disabled)
                    .on_click(cx.listener(|this, _, w, cx| this.import(false, w, cx))),
            )
            .child(
                tool("clear-voice", "close", "参照解除")
                    .disabled_with_cursor(disabled)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.toggle_chara_voice(None);
                        this.autosave_chara();
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn chara_preview(&self, chara: &Chara, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.state.busy;
        let saved = self
            .state
            .snapshot
            .chara_previews
            .contains(&chara.id)
            .then(|| chara.id.clone());
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.sample_text_field())
            .child(self.preview_settings(cx))
            .child(
                button("preview", "サンプル生成")
                    .primary()
                    .bg(self.accent())
                    .icon(icon("wand"))
                    .disabled_with_cursor(disabled || !self.can_generate())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.preview();
                        cx.notify();
                    })),
            )
            .when_some(saved, |el, id| el.child(self.saved_preview_actions(id, cx)))
            .when(
                self.playback.is_some()
                    && matches!(self.audio_id, Some(AudioSource::CharaPreview(_))),
                |el| el.child(self.saved_preview_player(cx)),
            )
            .into_any_element()
    }

    fn sample_text_field(&self) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.chara_locked;
        div()
            .text_xs()
            .text_color(p.muted)
            .child(
                Textarea::new(&self.fields.sample)
                    .h(px(70.))
                    .bg(p.control)
                    .disabled(disabled),
            )
            .into_any_element()
    }

    fn preview_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        div()
            .border_1()
            .border_color(p.line)
            .rounded(px(8.))
            .child(
                button("preview-settings", "生成設定")
                    .ghost()
                    .w_full()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.preview_form.expanded = !this.preview_form.expanded;
                        cx.notify();
                    })),
            )
            .when(self.preview_form.expanded, |el| {
                el.child(self.speed_controls(true))
                    .child(self.advanced_controls(true, cx))
            })
            .into_any_element()
    }

    fn saved_preview_actions(&self, id: String, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.state.busy;
        let export = id.clone();
        div()
            .flex()
            .gap_2()
            .child(
                tool("saved-preview", "play", "保存済みサンプルを聴く")
                    .disabled_with_cursor(disabled)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.start_audio(AudioSource::CharaPreview(id.clone()));
                        cx.notify();
                    })),
            )
            .child(
                tool("export-saved-preview", "download", "サンプル保存")
                    .disabled_with_cursor(disabled)
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.export(AudioSource::CharaPreview(export.clone()), w, cx)
                    })),
            )
            .into_any_element()
    }

    fn saved_preview_player(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                tool("saved-toggle", "pause", "再生 / 一時停止").on_click(cx.listener(
                    |this, _, _, cx| {
                        if let Some(p) = &this.playback {
                            p.toggle();
                        }
                        cx.notify();
                    },
                )),
            )
            .child(Slider::new(&self.seek).cursor_pointer().flex_1())
            .into_any_element()
    }
}
