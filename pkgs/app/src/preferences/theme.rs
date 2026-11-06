//! Application-wide theme and ordered settings writer, shared by workspace windows.
use gpui_kit::{App, Global};
struct AppTheme(pub bool);
struct ThemeWriter(pub super::SettingsWriter);
impl Global for ThemeWriter {}
impl Global for AppTheme {}

pub(crate) fn initialize(cx: &mut App) -> anyhow::Result<()> {
    cx.set_global(ThemeWriter(super::SettingsWriter::start()?));
    cx.set_global(AppTheme(super::Settings::load().light));
    Ok(())
}
pub(crate) fn light(cx: &App) -> bool {
    cx.global::<AppTheme>().0
}
pub(crate) fn toggle(cx: &mut App) -> bool {
    let light = !light(cx);
    cx.global_mut::<AppTheme>().0 = light;
    light
}
pub(crate) fn save(cx: &App, light: bool) -> anyhow::Result<()> {
    cx.global::<ThemeWriter>().0.save(light)
}
pub(crate) fn take_error(cx: &App) -> Option<String> {
    cx.global::<ThemeWriter>().0.errors.try_iter().last()
}
