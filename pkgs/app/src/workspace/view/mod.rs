use crate::audio::Playback;
use crate::preferences::Settings;
use crate::workspace::service::Service;
use crate::workspace::state::WorkspaceState;
use gpui_kit::component::color_picker::ColorPickerState;
use gpui_kit::component::input::{InputState, TextareaState};
use gpui_kit::component::slider::SliderState;
use gpui_kit::*;

struct Fields {
    text: Entity<TextareaState>,
    description: Entity<TextareaState>,
    prompt: Entity<InputState>,
    seed: Entity<InputState>,
    preview_seed: Entity<InputState>,
    preview_steps: Entity<InputState>,
    steps: Entity<InputState>,
    name: Entity<InputState>,
    sample: Entity<TextareaState>,
    color: Entity<ColorPickerState>,
}
struct GenerationSliders {
    speed: Entity<SliderState>,
    text: Entity<SliderState>,
    speaker: Entity<SliderState>,
    caption: Entity<SliderState>,
}
impl GenerationSliders {
    fn iter(
        &self,
    ) -> [(
        crate::workspace::state::GenerationField,
        &Entity<SliderState>,
    ); 4] {
        use crate::workspace::state::GenerationField::*;
        [
            (Speed, &self.speed),
            (TextGuidance, &self.text),
            (SpeakerGuidance, &self.speaker),
            (CaptionGuidance, &self.caption),
        ]
    }
}
pub(crate) struct Studio {
    state: WorkspaceState,
    service: Option<Service>,
    inference: Option<std::sync::Arc<crate::synthesis::Inference>>,
    fields: Fields,
    sliders: GenerationSliders,
    preview_sliders: GenerationSliders,
    seek: Entity<SliderState>,
    subscriptions: Vec<Subscription>,
    advanced: bool,
    preview_advanced: bool,
    line_scroll: ScrollHandle,
    line_drag_scroll: gpui_kit::component::scroll::AutoScroll,
    settings: Settings,
    playback: Option<Playback>,
    audio_id: Option<crate::domain::AudioSource>,
    sync: bool,
    focus: FocusHandle,
    menu_focus: FocusHandle,
    menu_trigger_focus: FocusHandle,
    menu_open: bool,
    file_trigger_focus: FocusHandle,
    menu_index: usize,
    dialog_open: bool,
    model_dialog_open: bool,
    model_picker_open: bool,
    model_error: Option<String>,
    model_trigger_focus: FocusHandle,
}

mod bindings;
mod controller;
mod dialogs;
mod editing;
mod playback;

mod appearance;

mod chara_panel;
mod footer;

mod app_dialog;
mod app_menu;
mod header;
mod model_settings;

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

mod components;
mod generation_controls;
mod layout;
mod lines;
mod player;
