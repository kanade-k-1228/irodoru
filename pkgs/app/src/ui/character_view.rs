use super::appearance::{button, icon, tool};
use super::*;
use gpui_kit::component::Sizable;
impl Studio {
    pub(super) fn character_editor(&self, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let d = self.draft.as_ref().unwrap();
        let p = self.palette();
        let disabled = self.busy;
        let preview = d.preview.clone();
        let saved =
            d.id.as_ref()
                .filter(|id| self.snapshot.character_previews.contains(*id))
                .cloned();
        let delete = d.id.clone();
        let name = d.input.name.clone();
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
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
                    .child(div().flex_1().child(if d.id.is_some() {
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
                            .disabled(disabled || d.input.name.trim().is_empty())
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(d) = &this.draft {
                                    let command = Command::SaveCharacter {
                                        id: d.id.clone(),
                                        input: d.input.clone(),
                                        preview: d.preview.clone(),
                                    };
                                    this.saving_draft = true;
                                    this.send(command, true);
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("character-form")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .flex()
                    .when(narrow, |el| el.flex_col())
                    .gap_4()
                    .child(
                        div()
                            .flex_1()
                            .when(narrow, |el| el.flex_none().w_full())
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        button("import-icon", "")
                                            .size(px(44.))
                                            .when(
                                                d.input.icon.as_ref().is_none_or(|i| i.is_empty()),
                                                |el| {
                                                    el.child(
                                                        div().text_xl().child(
                                                            d.input
                                                                .name
                                                                .chars()
                                                                .next()
                                                                .unwrap_or('？')
                                                                .to_string(),
                                                        ),
                                                    )
                                                },
                                            )
                                            .when_some(
                                                super::views::character_icon(
                                                    d.input.icon.as_deref(),
                                                ),
                                                |el, image| el.child(img(image).size(px(40.))),
                                            )
                                            .disabled(disabled)
                                            .tooltip("画像を選択")
                                            .on_click(cx.listener(|this, _, w, cx| {
                                                this.import(true, w, cx)
                                            })),
                                    )
                                    .when(
                                        d.input.icon.as_ref().is_some_and(|i| !i.is_empty()),
                                        |el| {
                                            el.child(
                                                tool("clear-icon", "close", "画像を削除")
                                                    .disabled(disabled)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        if let Some(d) = &mut this.draft {
                                                            d.input.icon = Some(String::new());
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
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(p.muted)
                                    .child("プロンプト")
                                    .child(
                                        Textarea::new(&self.fields.description)
                                            .h(px(112.))
                                            .bg(p.control)
                                            .disabled(disabled),
                                    ),
                            )
                            .child(
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
                                            .on_click(cx.listener(|this, _, w, cx| {
                                                this.import(false, w, cx)
                                            })),
                                    )
                                    .child(
                                        tool("clear-voice", "close", "参照解除")
                                            .disabled(disabled)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let Some(d) = &mut this.draft {
                                                    d.input.voice = None;
                                                }
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .child(
                                div().text_xs().text_color(p.muted).child(
                                    d.input
                                        .voice
                                        .clone()
                                        .unwrap_or_else(|| "参照音声なし".into()),
                                ),
                            )
                            .children(self.snapshot.voices.iter().enumerate().map(|(i, v)| {
                                let voice = v.clone();
                                Button::new(("voice", i))
                                    .small()
                                    .label(v.clone())
                                    .disabled(disabled)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(d) = &mut this.draft {
                                            d.input.voice = Some(voice.clone());
                                        }
                                        cx.notify();
                                    }))
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .when(narrow, |el| el.flex_none().w_full())
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(p.muted)
                                    .child("サンプル文")
                                    .child(
                                        Textarea::new(&self.fields.sample)
                                            .h(px(70.))
                                            .bg(p.control)
                                            .disabled(disabled),
                                    ),
                            )
                            .child(
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
                                        el.child(self.controls(cx))
                                            .child(self.advanced_controls(cx))
                                    }),
                            )
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
                                el.when_some(saved, |el, id| {
                                    let export = id.clone();
                                    el.child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .child(
                                                tool(
                                                    "saved-preview",
                                                    "play",
                                                    "保存済みサンプルを聴く",
                                                )
                                                .disabled(disabled)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.audio_id = None;
                                                    this.send(
                                                        Command::CharacterAudio(id.clone()),
                                                        true,
                                                    );
                                                    cx.notify();
                                                })),
                                            )
                                            .child(
                                                tool(
                                                    "export-saved-preview",
                                                    "download",
                                                    "サンプル保存",
                                                )
                                                .disabled(disabled)
                                                .on_click(cx.listener(move |this, _, w, cx| {
                                                    this.export_character(export.clone(), w, cx)
                                                })),
                                            ),
                                    )
                                })
                            })
                            .when(self.playback.is_some() && self.audio_id.is_none(), |el| {
                                el.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            tool("saved-toggle", "pause", "再生 / 一時停止")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    if let Some(p) = &this.playback {
                                                        p.toggle();
                                                    }
                                                    cx.notify();
                                                })),
                                        )
                                        .child(Slider::new(&self.seek).flex_1()),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }
}
