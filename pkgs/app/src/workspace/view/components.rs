use super::Studio;
use super::appearance::{Palette, blend, chara_color};
use crate::domain::{Chara, InferenceStatus};
use base64::Engine as _;
use gpui_kit::component::Icon;
use gpui_kit::*;

/// Aligns both dialogue and append rows without wrapping their interactive avatars.
pub(super) fn speech_row(avatar: impl Styled + IntoElement, content: impl IntoElement) -> Div {
    div()
        .relative()
        .flex()
        .items_start()
        .gap_2()
        .child(avatar.mt(px(6.)))
        .child(content)
}

impl Studio {
    /// The same frame accepts an editable dialogue or an add-line button.
    pub(super) fn speech_bubble(&self, border: Hsla, content: impl Styled + IntoElement) -> Div {
        div()
            .relative()
            .flex_1()
            .min_w_0()
            .child(
                content
                    .min_h(px(44.))
                    .rounded(px(8.))
                    .border_2()
                    .border_color(border)
                    .bg(self.palette().control),
            )
            .child(
                div()
                    .absolute()
                    .left(px(-8.))
                    .top(px(12.))
                    .size(px(16.))
                    .child(
                        Icon::default()
                            .data(b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'><path fill='currentColor' d='M16 1H10L2 8L10 15H16Z'/></svg>" as &'static [u8])
                            .size(px(16.))
                            .text_color(self.palette().control),
                    )
                    .child(
                        Icon::default()
                            .data(b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'><path fill='none' stroke='currentColor' stroke-width='2' stroke-linejoin='round' d='M10 1L2 8L10 15'/></svg>" as &'static [u8])
                            .absolute()
                            .top_0()
                            .left_0()
                            .size(px(16.))
                            .text_color(border),
                    ),
            )
    }

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
            .child(if let Some(image) = self.chara_icon(chara) {
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
            })
    }
}
/// A decoded chara icon and the data URL it came from.
pub(super) struct CachedIcon {
    data: String,
    image: Option<std::sync::Arc<Image>>,
}
impl Studio {
    /// Decodes an icon once per data URL; every avatar asks on every frame.
    pub(super) fn chara_icon(&self, chara: Option<&Chara>) -> Option<std::sync::Arc<Image>> {
        let chara = chara?;
        let data = chara.icon.as_deref()?;
        let mut icons = self.icons.borrow_mut();
        if let Some(icon) = icons.get(&chara.id)
            && icon.data == data
        {
            return icon.image.clone();
        }
        let image = decode_icon(data);
        icons.retain(|id, _| self.state.snapshot.charas.iter().any(|c| &c.id == id));
        icons.insert(
            chara.id.clone(),
            CachedIcon {
                data: data.to_owned(),
                image: image.clone(),
            },
        );
        image
    }
}
fn decode_icon(data: &str) -> Option<std::sync::Arc<Image>> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.strip_prefix("data:image/png;base64,")?)
        .ok()?;
    Some(std::sync::Arc::new(Image::from_bytes(
        ImageFormat::Png,
        bytes,
    )))
}
