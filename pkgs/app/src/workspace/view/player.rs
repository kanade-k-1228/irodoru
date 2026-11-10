use super::Studio;
use super::appearance::tool;
use crate::domain::AudioSource;
use crate::workspace::protocol::Command;
use crate::workspace::view::appearance::DisabledWithCursor;
use gpui_kit::component::button::Button;
use gpui_kit::component::slider::Slider;
use gpui_kit::*;

impl Studio {
    pub(super) fn play(&mut self, source: AudioSource) {
        if self.audio_id.as_ref() == Some(&source)
            && let Some(p) = &self.playback
        {
            p.toggle();
            return;
        }
        self.start_audio(source);
    }
    pub(super) fn start_audio(&mut self, source: AudioSource) {
        self.playback = None;
        self.audio_id = Some(source.clone());
        self.send(Command::Audio(source));
    }
    pub(super) fn stop_audio(&mut self) {
        self.playback = None;
        self.audio_id = None;
    }
    pub(super) fn audio_play_button(
        &self,
        id: impl Into<ElementId>,
        source: Option<AudioSource>,
        cx: &mut Context<Self>,
    ) -> Button {
        let playing = source.is_some()
            && self.audio_id == source
            && self
                .playback
                .as_ref()
                .is_some_and(|player| player.playing());
        tool(
            id,
            if playing { "pause" } else { "play" },
            if source.is_none() {
                "音声を生成すると再生できます"
            } else {
                "再生 / 一時停止"
            },
        )
        .disabled_with_cursor(self.state.busy || source.is_none())
        .on_click(cx.listener(move |this, _, _, cx| {
            if let Some(source) = &source {
                this.play(source.clone());
                cx.notify();
            }
        }))
    }
    pub(super) fn audio_player_controls(
        &self,
        key: (&'static str, usize),
        source: AudioSource,
        duration: f64,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let active = self.audio_id.as_ref() == Some(&source);
        let player = self.playback.as_ref().filter(|_| active);
        let position = player.map(|p| p.position()).unwrap_or(0.);
        div()
            .flex()
            .flex_1()
            .flex_basis(px(140.))
            .min_w_0()
            .flex_wrap()
            .items_center()
            .gap_1()
            .text_xs()
            .text_color(p.muted)
            .child(self.audio_play_button(key, Some(source), cx))
            .child(
                div()
                    .w(px(36.))
                    .flex_shrink_0()
                    .text_right()
                    .child(time(position.into())),
            )
            .child(if active {
                Slider::new(&self.seek)
                    .cursor_pointer()
                    .flex_1()
                    .min_w(px(16.))
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .min_w(px(16.))
                    .h(px(4.))
                    .rounded_full()
                    .bg(p.control)
                    .child(
                        div()
                            .size(px(10.))
                            .mt(px(-3.))
                            .rounded_full()
                            .bg(self.accent()),
                    )
                    .into_any_element()
            })
            .child(div().w(px(36.)).flex_shrink_0().child(time(duration)))
            .into_any_element()
    }
}
fn time(seconds: f64) -> String {
    let seconds = seconds.max(0.) as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
