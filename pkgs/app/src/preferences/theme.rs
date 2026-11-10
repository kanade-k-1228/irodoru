//! Application-wide theme, shared by workspace windows.
use super::{Settings, Theme};
use anyhow::Context;
use gpui_kit::{App, Global};
struct AppTheme(Theme);
impl Global for AppTheme {}

pub fn initialize(cx: &mut App) {
    cx.set_global(AppTheme(Settings::load().theme));
}
pub(crate) fn light(cx: &App) -> bool {
    current(cx).is_light()
}
pub(crate) fn current(cx: &App) -> Theme {
    cx.global::<AppTheme>().0
}
pub(crate) fn toggle(cx: &mut App) -> Theme {
    let theme = match current(cx) {
        Theme::Light => Theme::Dark,
        Theme::Dark => Theme::Light,
    };
    cx.global_mut::<AppTheme>().0 = theme;
    theme
}
pub(crate) fn save(theme: Theme) -> anyhow::Result<()> {
    Settings { theme }
        .save()
        .context("テーマを保存できませんでした")
}
