//! Character editor composition, followed by each form section.
use super::Studio;
use super::appearance::{button, icon, tool};
use crate::workspace::protocol::Command;
use crate::workspace::state::Draft;
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
    pub(super) fn character_editor(&self, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let draft = self.state.draft.as_ref().unwrap();
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .child(self.character_toolbar(draft, cx))
            .child(self.character_form(draft, narrow, cx))
            .into_any_element()
    }

    fn character_toolbar(&self, draft: &Draft, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy;
        let delete = draft.id.clone();
        let name = draft.input.name.clone();
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_1()
            .border_b_1()
            .border_color(p.line)
            .text_xs()
            .text_color(p.muted)
            .child(div().flex_1().child(if draft.id.is_some() {
                "キャラ編集"
            } else {
                "キャラ追加"
            }))
            .when_some(delete, |el, id| {
                el.child(
                    tool("delete-character", "trash", "キャラを削除")
                        .bg(rgb(0xb52e42))
                        .text_color(rgb(0xffffff))
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, _, w, cx| {
                            this.delete_character(id.clone(), name.clone(), w, cx)
                        })),
                )
            })
            .child(
                tool("cancel-character", "close", "キャンセル")
                    .disabled(disabled)
                    .on_click(cx.listener(|this, _, w, cx| this.cancel_character(w, cx))),
            )
            .child(
                tool("save-character", "save", "キャラクターを保存")
                    .text_color(self.accent())
                    .disabled(disabled || draft.input.name.trim().is_empty())
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(draft) = &this.state.draft {
                            let command = Command::SaveCharacter {
                                id: draft.id.clone(),
                                input: draft.input.clone(),
                                preview: draft.preview.clone(),
                            };
                            this.state.saving_draft = true;
                            this.send(command);
                        }
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn character_form(&self, draft: &Draft, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("character-form")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_4()
            .flex()
            .when(narrow, |el| el.flex_col())
            .gap_4()
            .child(self.character_details(draft, narrow, cx))
            .child(self.character_preview(draft, narrow, cx))
            .into_any_element()
    }

    fn character_details(&self, draft: &Draft, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy;
        div()
            .flex_1()
            .when(narrow, |el| el.flex_none().w_full())
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.character_identity(draft, cx))
            .child(self.character_description())
            .child(self.reference_voice_actions(cx))
            .child(
                div().text_xs().text_color(p.muted).child(
                    draft
                        .input
                        .voice
                        .clone()
                        .unwrap_or_else(|| "参照音声なし".into()),
                ),
            )
            .children(self.state.snapshot.voices.iter().enumerate().map(|(i, v)| {
                let voice = v.clone();
                Button::new(("voice", i))
                    .small()
                    .label(v.clone())
                    .disabled(disabled)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(draft) = &mut this.state.draft {
                            draft.input.voice = Some(voice.clone());
                        }
                        cx.notify();
                    }))
            }))
            .into_any_element()
    }

    fn character_identity(&self, draft: &Draft, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy;
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(self.character_icon_button(draft, cx))
            .when(
                draft.input.icon.as_ref().is_some_and(|i| !i.is_empty()),
                |el| {
                    el.child(
                        tool("clear-icon", "close", "画像を削除")
                            .disabled(disabled)
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(draft) = &mut this.state.draft {
                                    draft.input.icon = Some(String::new());
                                }
                                cx.notify();
                            })),
                    )
                },
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_xs()
                    .text_color(p.muted)
                    .child("名前")
                    .child(
                        Input::new(&self.fields.name)
                            .small()
                            .bg(p.control)
                            .disabled(disabled),
                    ),
            )
            .child(
                div()
                    .w(px(32.))
                    .text_xs()
                    .text_color(p.muted)
                    .child("色")
                    .child(if disabled {
                        div()
                            .size(px(32.))
                            .rounded(px(8.))
                            .bg(self.accent())
                            .opacity(0.5)
                            .into_any_element()
                    } else {
                        ColorPicker::new(&self.fields.color)
                            .small()
                            .size(px(32.))
                            .accessibility_label("テーマカラー")
                            .into_any_element()
                    }),
            )
            .into_any_element()
    }

    fn character_icon_button(&self, draft: &Draft, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.state.busy;
        button("import-icon", "")
            .size(px(44.))
            .when(
                draft.input.icon.as_ref().is_none_or(|i| i.is_empty()),
                |el| {
                    el.child(
                        div()
                            .text_xl()
                            .child(draft.input.name.chars().next().unwrap_or('？').to_string()),
                    )
                },
            )
            .when_some(
                super::components::character_icon(draft.input.icon.as_deref()),
                |el, image| el.child(img(image).size(px(40.))),
            )
            .disabled(disabled)
            .tooltip("画像を選択")
            .on_click(cx.listener(|this, _, w, cx| this.import(true, w, cx)))
            .into_any_element()
    }

    fn character_description(&self) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy;
        div()
            .text_xs()
            .text_color(p.muted)
            .child("プロンプト")
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
        let disabled = self.state.busy;
        div()
            .flex()
            .items_center()
            .gap_2()
            .text_xs()
            .text_color(p.muted)
            .child("参照音声")
            .child(
                tool("import-voice", "folder", "音声を取り込む")
                    .disabled(disabled)
                    .on_click(cx.listener(|this, _, w, cx| this.import(false, w, cx))),
            )
            .child(
                tool("clear-voice", "close", "参照解除")
                    .disabled(disabled)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(draft) = &mut this.state.draft {
                            draft.input.voice = None;
                        }
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn character_preview(&self, draft: &Draft, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.state.busy;
        let preview = draft.preview.clone();
        let saved = draft
            .id
            .as_ref()
            .filter(|id| self.state.snapshot.character_previews.contains(*id))
            .cloned();
        div()
            .flex_1()
            .when(narrow, |el| el.flex_none().w_full())
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
                    .disabled(disabled || !self.can_generate())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.preview();
                        cx.notify();
                    })),
            )
            .when_some(preview.clone(), |el, id| {
                el.child(
                    self.audio_player(
                        ("preview-audio", 0),
                        id,
                        true,
                        self.playback
                            .as_ref()
                            .map(|p| p.duration as f64)
                            .unwrap_or(0.),
                        cx,
                    ),
                )
            })
            .when(preview.is_none(), |el| {
                el.when_some(saved, |el, id| el.child(self.saved_preview_actions(id, cx)))
            })
            .when(self.playback.is_some() && self.audio_id.is_none(), |el| {
                el.child(self.saved_preview_player(cx))
            })
            .into_any_element()
    }

    fn sample_text_field(&self) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy;
        div()
            .text_xs()
            .text_color(p.muted)
            .child("サンプル文")
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
                        this.advanced = !this.advanced;
                        cx.notify();
                    })),
            )
            .when(self.advanced, |el| {
                el.child(self.speed_controls())
                    .child(self.advanced_controls(cx))
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
                    .disabled(disabled)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.audio_id = None;
                        this.send(Command::CharacterAudio(id.clone()));
                        cx.notify();
                    })),
            )
            .child(
                tool("export-saved-preview", "download", "サンプル保存")
                    .disabled(disabled)
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.export_character(export.clone(), w, cx)
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
            .child(Slider::new(&self.seek).flex_1())
            .into_any_element()
    }
}
