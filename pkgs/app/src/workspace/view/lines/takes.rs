use crate::domain::{AudioSource, Take};
use crate::workspace::view::Studio;
use crate::workspace::view::audio_item::AudioItem;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(in crate::workspace::view) fn take_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let takes = self
            .state
            .snapshot
            .editor
            .lines
            .iter()
            .find(|line| Some(&line.id) == self.state.selected.as_ref())
            .map(|line| line.takes.as_slice())
            .unwrap_or_default();
        div()
            .w(relative(0.5))
            .flex_shrink_0()
            .min_w_0()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .when(takes.is_empty(), |el| {
                el.child(
                    div()
                        .text_xs()
                        .text_color(p.muted)
                        .child("音声を生成すると、ここにテイクが表示されます。"),
                )
            })
            .children(
                takes
                    .iter()
                    .filter_map(|id| self.state.snapshot.takes.iter().find(|take| &take.id == id))
                    .enumerate()
                    .map(|(i, take)| self.take_item(i, take, cx)),
            )
            .into_any_element()
    }

    fn take_item(&self, i: usize, take: &Take, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.state.snapshot.editor.lines.iter().any(|line| {
            Some(&line.id) == self.state.selected.as_ref()
                && line.take_id.as_ref() == Some(&take.id)
        });
        self.audio_item(
            AudioItem {
                key: ("take", i),
                source: AudioSource::Take(take.id.clone()),
                seconds: take.audio_seconds,
                selected: Some(selected),
                label: format!(
                    "テイク {} · シード {} · {} ステップ · {:.1}秒で生成",
                    i + 1,
                    take.conditions.seed,
                    take.conditions.num_steps,
                    take.elapsed_ms as f64 / 1000.
                ),
            },
            cx,
        )
    }
}
