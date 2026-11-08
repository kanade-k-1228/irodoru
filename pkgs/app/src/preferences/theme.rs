//! Application-wide theme and ordered settings writer, shared by workspace windows.
use super::Theme;
use gpui_kit::{App, Global};
struct AppTheme(pub Theme);
struct ThemeWriter(pub super::SettingsWriter);
impl Global for ThemeWriter {}
impl Global for AppTheme {}

pub(crate) fn initialize(cx: &mut App) -> anyhow::Result<()> {
    cx.set_global(ThemeWriter(super::SettingsWriter::start()?));
    cx.set_global(AppTheme(super::Settings::load().theme));
    Ok(())
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
pub(crate) fn save(cx: &App, theme: Theme) -> anyhow::Result<()> {
    cx.global::<ThemeWriter>().0.save(theme)
}
pub(crate) fn take_error(cx: &App) -> Option<String> {
    cx.global::<ThemeWriter>().0.errors.try_iter().last()
}
