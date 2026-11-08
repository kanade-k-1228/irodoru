use super::Studio;
use super::appearance::tool;
use gpui_kit::component::Disableable;
use gpui_kit::component::slider::Slider;
use gpui_kit::*;

impl Studio {
    pub(super) fn audio_player(
        &self,
        key: (&'static str, usize),
        id: String,
        preview: bool,
        duration: f64,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let active = self.audio_id.as_deref() == Some(format!("{preview}:{id}").as_str());
        let player = self.playback.as_ref().filter(|_| active);
        let position = player.map(|p| p.position()).unwrap_or(0.);
        let playing = player.is_some_and(|p| p.playing());
        let export = id.clone();
        div()
            .flex()
            .items_center()
            .gap_1()
            .text_xs()
            .text_color(p.muted)
            .child(
                tool(
                    (key.0, key.1 * 2),
                    if playing { "pause" } else { "play" },
                    "再生 / 一時停止",
                )
                .disabled(self.state.busy)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.play(id.clone(), preview);
                    cx.notify();
                })),
            )
            .child(div().w(px(36.)).text_right().child(time(position.into())))
            .child(if active {
                Slider::new(&self.seek).flex_1().into_any_element()
            } else {
                div()
                    .flex_1()
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
            .child(div().w(px(36.)).child(time(duration)))
            .child(
                tool((key.0, key.1 * 2 + 1), "download", "WAVを保存")
                    .disabled(self.state.busy)
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.export(export.clone(), preview, w, cx)
                    })),
            )
            .into_any_element()
    }
}
fn time(seconds: f64) -> String {
    let seconds = seconds.max(0.) as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
