use crate::domain::{AudioSource, Chara};
use crate::workspace::view::Studio;
use crate::workspace::view::appearance::{DisabledWithCursor, button, icon};
use crate::workspace::view::audio_item::AudioItem;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(super) fn chara_references(&self, chara: &Chara, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let voices: Vec<_> = chara
            .voice_ids
            .iter()
            .filter_map(|id| {
                self.state
                    .snapshot
                    .voices
                    .iter()
                    .find(|entry| entry.voice.id() == id)
            })
            .collect();
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            .border_l_1()
            .border_color(p.line)
            .pl_4()
            .child(
                div()
                    .id("reference-voices")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(self.chara_form_wheel.handle())
                    .on_scroll(self.chara_form_wheel.listener(cx.entity_id()))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .when(voices.is_empty(), |el| {
                        el.child(
                            div()
                                .text_xs()
                                .text_color(p.muted)
                                .child("音声ファイルを取り込んで参照音声を追加"),
                        )
                    })
                    .children(voices.into_iter().enumerate().map(|(i, entry)| {
                        let id = entry.voice.id().to_owned();
                        self.audio_item(
                            AudioItem {
                                key: ("reference", i),
                                selected: None,
                                source: AudioSource::Reference(id),
                                seconds: entry.audio_seconds,
                                label: entry.voice.label(),
                            },
                            cx,
                        )
                    }))
                    .child(
                        button("import-voice", "参照音声を追加")
                            .icon(icon("plus"))
                            .flex_shrink_0()
                            .disabled_with_cursor(self.state.busy || self.state.chara_locked)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.import(false, window, cx);
                            })),
                    ),
            )
            .into_any_element()
    }
}
