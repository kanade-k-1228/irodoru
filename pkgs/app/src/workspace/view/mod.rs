use crate::audio::Playback;
use crate::preferences::Settings;
use crate::workspace::service::Service;
use crate::workspace::state::WorkspaceState;
use gpui_kit::component::color_picker::ColorPickerState;
use gpui_kit::component::input::{InputState, TextareaState};
use gpui_kit::component::slider::SliderState;
use gpui_kit::*;

#[derive(Clone)]
struct DragRow(String);
impl Render for DragRow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p_2().bg(rgb(0x41a2ec)).child("行を移動")
    }
}
struct Fields {
    text: Entity<TextareaState>,
    description: Entity<TextareaState>,
    prompt: Entity<InputState>,
    seed: Entity<InputState>,
    steps: Entity<InputState>,
    name: Entity<InputState>,
    sample: Entity<TextareaState>,
    color: Entity<ColorPickerState>,
}
pub(crate) struct Studio {
    state: WorkspaceState,
    service: Option<Service>,
    fields: Fields,
    sliders: Vec<Entity<SliderState>>,
    seek: Entity<SliderState>,
    subscriptions: Vec<Subscription>,
    advanced: bool,
    script_scroll: ScrollHandle,
    settings: Settings,
    playback: Option<Playback>,
    audio_id: Option<String>,
    sync: bool,
    focus: FocusHandle,
    menu_focus: FocusHandle,
    menu_trigger_focus: FocusHandle,
    menu_open: bool,
    menu_index: usize,
    dialog_open: bool,
}

mod bindings;
mod controller;
mod dialogs;
mod editing;
mod playback;

mod appearance;

mod character_view;

mod app_menu;

pub(crate) mod actions;
pub(crate) enum WindowRequest {
    New,
}
impl EventEmitter<WindowRequest> for Studio {}
impl Studio {
    pub(crate) fn report_error(&mut self, error: String, cx: &mut Context<Self>) {
        self.state.error = Some(error);
        cx.notify();
    }
}

mod characters;
mod components;
mod generation_controls;
mod layout;
mod line_editor;
mod player;
mod script;
