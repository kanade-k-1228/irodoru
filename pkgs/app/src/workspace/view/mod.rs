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
    color: Entity<ColorPickerState>,
}
/// Inputs for the selected line's generation settings.
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
#[derive(PartialEq)]
struct ModelSnapshot {
    status: crate::domain::InferenceStatus,
    repository: Option<String>,
    source: Option<String>,
    providers: Vec<irodoru_engine::ProviderStatus>,
}

pub(crate) struct Studio {
    state: WorkspaceState,
    service: Option<Service>,
    inference: Option<std::sync::Arc<crate::synthesis::Inference>>,
    fields: Fields,
    line_form: GenerationForm,
    seek: Entity<SliderState>,
    subscriptions: Vec<Subscription>,
    line_scroll: ScrollHandle,
    line_drag_scroll: gpui_kit::component::scroll::AutoScroll,
    line_wheel: SmoothScroll,
    chara_list_wheel: SmoothScroll,
    chara_form_wheel: SmoothScroll,
    icons: std::cell::RefCell<std::collections::HashMap<String, CachedIcon>>,
    model_seen: Option<ModelSnapshot>,
    settings: Settings,
    playback: Option<Playback>,
    audio_id: Option<crate::domain::AudioSource>,
    sync: bool,
    focus: FocusHandle,
    menu_focus: FocusHandle,
    menu_trigger_focus: FocusHandle,
    file_trigger_focus: FocusHandle,
    menu_index: usize,
    modal: Option<modal::Modal>,
    model_picker_open: bool,
    model_error: Option<String>,
    model_details_open: bool,
    model_trigger_focus: FocusHandle,
    export_trigger_focus: FocusHandle,
    export_naming: crate::project::ExportNaming,
}

mod bindings;
mod controller;
mod dialogs;
mod editing;

mod appearance;

mod chara_panel;
mod footer;

mod header;
mod modal;

pub mod actions;
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

mod audio_item;
mod components;
mod generation_controls;
mod layout;
mod lines;
mod player;
mod smooth_scroll;
use components::CachedIcon;
use smooth_scroll::SmoothScroll;
