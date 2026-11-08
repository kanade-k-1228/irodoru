use super::Studio;
use super::appearance::{Palette, blend, character_color};
use crate::domain::{Character, InferenceStatus};
use base64::Engine as _;
use gpui_kit::*;

impl Studio {
    pub(super) fn palette(&self) -> Palette {
        Palette::new(self.settings.theme.is_light())
    }
    pub(super) fn accent(&self) -> Hsla {
        let color = self
            .state
            .draft
            .as_ref()
            .and_then(|d| d.input.color.as_deref())
            .or_else(|| {
                self.state
                    .snapshot
                    .characters
                    .iter()
                    .find(|c| c.id == self.state.snapshot.editor.active_character)
                    .map(|c| c.color.as_str())
            });
        character_color(color, self.palette().accent)
    }
    pub(super) fn can_generate(&self) -> bool {
        self.service
            .as_ref()
            .is_some_and(|s| matches!(s.status(), InferenceStatus::Ready))
            && self.state.status.id.is_some()
    }
    pub(super) fn avatar(&self, character: Option<&Character>, size: f32) -> AnyElement {
        let p = self.palette();
        let color = character_color(character.map(|c| c.color.as_str()), p.accent);
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
                if let Some(image) = character_icon(character.and_then(|c| c.icon.as_deref())) {
                    img(image).size_full().into_any_element()
                } else {
                    div()
                        .child(
                            character
                                .and_then(|c| c.name.chars().next())
                                .map(|c| c.to_string())
                                .unwrap_or_else(|| "?".into()),
                        )
                        .into_any_element()
                },
            )
            .into_any_element()
    }
}
pub(super) fn character_icon(data: Option<&str>) -> Option<std::sync::Arc<Image>> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data?.strip_prefix("data:image/png;base64,")?)
        .ok()?;
    Some(std::sync::Arc::new(Image::from_bytes(
        ImageFormat::Png,
        bytes,
    )))
}
