use super::Studio;
use super::appearance::{Palette, blend, chara_color};
use crate::domain::{Chara, InferenceStatus};
use base64::Engine as _;
use gpui_kit::*;

impl Studio {
    pub(super) fn palette(&self) -> Palette {
        Palette::new(self.settings.theme.is_light())
    }
    pub(super) fn accent(&self) -> Hsla {
        let color = self.state.active_chara().map(|chara| chara.color.as_str());
        chara_color(color, self.palette().accent)
    }
    pub(super) fn can_generate(&self) -> bool {
        self.inference
            .as_ref()
            .is_some_and(|s| matches!(s.status(), InferenceStatus::Ready))
            && self.state.status.id.is_some()
    }
    pub(super) fn avatar(&self, chara: Option<&Chara>, size: f32) -> Div {
        let p = self.palette();
        let color = chara_color(chara.map(|c| c.color.as_str()), p.accent);
        div()
            .size(px(size))
            .flex_shrink_0()
            .rounded_full()
            .border_1()
            .border_color(blend(color, p.line, 0.65))
            .bg(blend(color, p.panel, 0.18))
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .text_sm()
            .child(
                if let Some(image) = chara_icon(chara.and_then(|c| c.icon.as_deref())) {
                    img(image).size_full().rounded_full().into_any_element()
                } else {
                    div()
                        .child(
                            chara
                                .and_then(|c| c.name.chars().next())
                                .map(|c| c.to_string())
                                .unwrap_or_else(|| "?".into()),
                        )
                        .into_any_element()
                },
            )
    }
}
pub(super) fn chara_icon(data: Option<&str>) -> Option<std::sync::Arc<Image>> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data?.strip_prefix("data:image/png;base64,")?)
        .ok()?;
    Some(std::sync::Arc::new(Image::from_bytes(
        ImageFormat::Png,
        bytes,
    )))
}
