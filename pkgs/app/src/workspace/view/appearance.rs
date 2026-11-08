//! Original studio palette and Lucide-style line icons, shared by native views.
use gpui_kit::component::{Icon, Sizable};
use gpui_kit::component::{
    Theme,
    button::{Button, ButtonVariants},
};
use gpui_kit::*;
#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub surface: Hsla,
    pub panel: Hsla,
    pub control: Hsla,
    pub hover: Hsla,
    pub line: Hsla,
    pub ink: Hsla,
    pub muted: Hsla,
    pub accent: Hsla,
    pub accent_hover: Hsla,
    pub soft: Hsla,
    pub on_accent: Hsla,
    pub toolbar: Hsla,
    pub warning: Hsla,
    pub warning_soft: Hsla,
}
impl Palette {
    pub fn new(light: bool) -> Self {
        let c = if light {
            [
                0xf0f2f5, 0xffffff, 0xf5f7fa, 0xe8edf4, 0xd5dbe3, 0x28313d, 0x647180, 0x1979b8,
                0x16699f, 0xdceffd, 0xffffff, 0xdceafa, 0xb93850, 0xffebef,
            ]
        } else {
            [
                0x25282d, 0x2a2e33, 0x363a3f, 0x42484f, 0x3b3e43, 0xfbeeea, 0xb0b2b7, 0x41a2ec,
                0x65b4f0, 0x184d7c, 0x112c40, 0x184d7c, 0xf27483, 0x412d36,
            ]
        };
        let [
            surface,
            panel,
            control,
            hover,
            line,
            ink,
            muted,
            accent,
            accent_hover,
            soft,
            on_accent,
            toolbar,
            warning,
            warning_soft,
        ] = c.map(|v| rgb(v).into());
        Self {
            surface,
            panel,
            control,
            hover,
            line,
            ink,
            muted,
            accent,
            accent_hover,
            soft,
            on_accent,
            toolbar,
            warning,
            warning_soft,
        }
    }
}
pub(super) fn apply(light: bool, cx: &mut App) {
    let p = Palette::new(light);
    Theme::update(cx, |theme| {
        theme.radius = px(8.);
        theme.shadow = false;
        theme.font_size = px(16.);
        let t = &mut theme.colors;
        t.background = p.surface;
        t.overlay = if light {
            rgba(0x192c443d).into()
        } else {
            rgba(0x0000008c).into()
        };
        t.foreground = p.ink;
        t.border = p.line;
        t.input = p.line;
        t.muted = p.control;
        t.muted_foreground = p.muted;
        t.accent = p.soft;
        t.accent_foreground = p.ink;
        t.primary = p.accent;
        t.primary_foreground = p.on_accent;
        t.primary_hover = p.accent_hover;
        t.button = p.control;
        t.button_foreground = p.ink;
        t.button_hover = p.hover;
        t.button_active = p.hover;
        t.button_primary = p.accent;
        t.button_primary_foreground = p.on_accent;
        t.button_primary_hover = p.accent_hover;
        t.selection = p.soft;
        t.ring = p.accent;
        t.caret = p.ink;
        t.slider_bar = p.accent;
        t.slider_thumb = p.accent;
    });
}
pub(super) fn icon(name: &str) -> Icon {
    let data: &[u8] = match name {
        "chevron-down" => include_bytes!("../../../icons/ui/chevron-down.svg"),
        "minus" => include_bytes!("../../../icons/ui/minus.svg"),
        "square" => include_bytes!("../../../icons/ui/square.svg"),
        "audio" => include_bytes!("../../../icons/ui/audio.svg"),
        "new" => include_bytes!("../../../icons/ui/new.svg"),
        "folder" => include_bytes!("../../../icons/ui/folder.svg"),
        "save" => include_bytes!("../../../icons/ui/save.svg"),
        "sun" => include_bytes!("../../../icons/ui/sun.svg"),
        "moon" => include_bytes!("../../../icons/ui/moon.svg"),
        "plus" => include_bytes!("../../../icons/ui/plus.svg"),
        "edit" => include_bytes!("../../../icons/ui/edit.svg"),
        "trash" => include_bytes!("../../../icons/ui/trash.svg"),
        "close" => include_bytes!("../../../icons/ui/close.svg"),
        "grip" => include_bytes!("../../../icons/ui/grip.svg"),
        "up" => include_bytes!("../../../icons/ui/up.svg"),
        "down" => include_bytes!("../../../icons/ui/down.svg"),
        "play" => include_bytes!("../../../icons/ui/play.svg"),
        "pause" => include_bytes!("../../../icons/ui/pause.svg"),
        "download" => include_bytes!("../../../icons/ui/download.svg"),
        "settings" => include_bytes!("../../../icons/ui/settings.svg"),
        "dice" => include_bytes!("../../../icons/ui/dice.svg"),
        "wand" => include_bytes!("../../../icons/ui/wand.svg"),
        "copy" => include_bytes!("../../../icons/ui/copy.svg"),
        "alert" => include_bytes!("../../../icons/ui/alert.svg"),
        _ => include_bytes!("../../../icons/ui/alert.svg"),
    };
    Icon::default().data(data).size(px(16.))
}

pub(super) fn tool(id: impl Into<ElementId>, name: &str, label: &'static str) -> Button {
    Button::new(id)
        .cursor_pointer()
        .ghost()
        .icon(icon(name))
        .tooltip(label)
        .w(px(28.))
        .h(px(28.))
        .rounded(px(8.))
}
pub(super) fn button(id: impl Into<ElementId>, label: &'static str) -> Button {
    Button::new(id)
        .cursor_pointer()
        .small()
        .label(label)
        .h(px(32.))
        .rounded(px(8.))
}
pub(super) fn blend(a: Hsla, b: Hsla, f: f32) -> Hsla {
    let a: Rgba = a.into();
    let b: Rgba = b.into();
    Rgba {
        r: a.r * f + b.r * (1. - f),
        g: a.g * f + b.g * (1. - f),
        b: a.b * f + b.b * (1. - f),
        a: 1.,
    }
    .into()
}
pub(super) fn character_color(color: Option<&str>, fallback: Hsla) -> Hsla {
    color
        .and_then(|s| u32::from_str_radix(s.trim_start_matches('#'), 16).ok())
        .map(|v| rgb(v).into())
        .unwrap_or(fallback)
}
