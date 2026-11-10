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
    name: Entity<InputState>,
    sample: Entity<TextareaState>,
    color: Entity<ColorPickerState>,
}
/// Inputs for one set of generation settings: the selected line's or the chara preview's.
struct GenerationForm {
    seed: Entity<InputState>,
    steps: Entity<InputState>,
    speed: Entity<SliderState>,
    text: Entity<SliderState>,
    speaker: Entity<SliderState>,
    caption: Entity<SliderState>,
    expanded: bool,
}
impl GenerationForm {
    fn sliders(
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
    line_form: GenerationForm,
    preview_form: GenerationForm,
    seek: Entity<SliderState>,
    subscriptions: Vec<Subscription>,
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
    modal: Option<app_dialog::Modal>,
    model_picker_open: bool,
    model_error: Option<String>,
    model_trigger_focus: FocusHandle,
}

mod bindings;
mod controller;
mod dialogs;
mod editing;

mod appearance;

mod chara_panel;
mod footer;

mod app_dialog;
mod app_menu;
mod header;
mod model_settings;

pub mod actions;
pub(crate) enum WindowRequest {
    New,
}
impl EventEmitter<WindowRequest> for Studio {}
impl Studio {
    fn form(&self, preview: bool) -> &GenerationForm {
        if preview {
            &self.preview_form
        } else {
            &self.line_form
        }
    }
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
