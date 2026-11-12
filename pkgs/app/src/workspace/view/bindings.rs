use super::{Fields, GenerationForm, Studio, appearance};
use crate::domain::GenerationSettings;
use crate::preferences::Settings;
use crate::workspace::protocol::Command;
use crate::workspace::service::Service;
use crate::workspace::state::{Field, GenerationField, WorkspaceState};
use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::component::slider::{SliderEvent, SliderState, SliderValue};
use gpui_kit::*;
use std::time::Duration;

impl Studio {
    pub(crate) fn new(
        path: Option<std::path::PathBuf>,
        inference: Result<std::sync::Arc<crate::synthesis::Inference>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = |placeholder: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let fields = Fields {
            text: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .placeholder("セリフを入力")
                    .auto_grow(1, 8)
                    .submit_on_enter(true)
            }),
            description: cx
                .new(|cx| TextareaState::new(window, cx).placeholder("声の特徴・話し方")),
            prompt: input("追加プロンプト", window, cx),
            name: input("キャラクター名", window, cx),
            color: cx.new(|cx| ColorPickerState::new(window, cx).default_value(rgb(0x41a2ec))),
        };
        let line_form = make_form(window, cx);
        let seek = cx.new(|_| SliderState::new().min(0.0).max(1.0).step(0.001));
        let mut subscriptions = Vec::new();
        for (entity, field) in [
            (&fields.prompt, Field::Prompt),
            (&line_form.seed, Field::Seed),
            (&line_form.steps, Field::Steps),
            (&fields.name, Field::Name),
        ] {
            subscriptions.push(cx.subscribe_in(
                entity,
                window,
                move |this, input, event, _, cx| {
                    if matches!(event, InputEvent::Change) && !this.sync && !this.state.busy {
                        this.changed(field, input.read(cx).value().to_string());
                        cx.notify();
                    }
                },
            ));
        }
        subscriptions.push(
            cx.subscribe_in(&fields.color, window, |this, _, event, _, cx| {
                if let ColorPickerEvent::Change(Some(color)) = event
                    && !this.sync
                    && !this.state.busy
                {
                    let c: Rgba = (*color).into();
                    this.state.set_chara_color(format!(
                        "#{:02x}{:02x}{:02x}",
                        (c.r * 255.).round() as u8,
                        (c.g * 255.).round() as u8,
                        (c.b * 255.).round() as u8
                    ));
                    this.autosave_chara();
                    cx.notify();
                }
            }),
        );
        subscriptions.push(cx.subscribe_in(
            &fields.text,
            window,
            |this, input, event, window, cx| {
                if matches!(event, InputEvent::Change) && !this.sync && !this.state.busy {
                    let value = input.read(cx).value().to_string();
                    this.changed(Field::Text, value);
                    cx.notify();
                }
                if let InputEvent::PressEnter {
                    secondary: true,
                    shift: false,
                } = event
                    && !this.sync
                    && !this.state.busy
                    && this.can_generate()
                    && !input.read(cx).value().trim().is_empty()
                    && input.update(cx, |state, cx| {
                        state.marked_text_range(window, cx).is_none()
                    })
                {
                    this.generate(false, window, cx);
                }
            },
        ));
        subscriptions.push(cx.subscribe_in(
            &fields.description,
            window,
            |this, input, event, _, cx| {
                if matches!(event, InputEvent::Change) && !this.sync && !this.state.busy {
                    this.changed(Field::Description, input.read(cx).value().to_string());
                    cx.notify();
                }
            },
        ));
        for (field, slider) in line_form.sliders() {
            subscriptions.push(
                cx.subscribe_in(slider, window, move |this, _, event, _, cx| {
                    if let SliderEvent::Change(SliderValue::Single(value)) = event
                        && !this.sync
                        && !this.state.busy
                    {
                        let value = if matches!(field, GenerationField::Speed) {
                            value.exp2()
                        } else {
                            *value
                        };
                        if this.state.change_generation(field, value) {
                            this.autosave();
                        }
                        cx.notify();
                    }
                }),
            );
        }
        subscriptions.push(cx.subscribe_in(&seek, window, |this, _, event, _, cx| {
            if let SliderEvent::Change(SliderValue::Single(value)) = event
                && let Some(player) = &this.playback
                && let Err(e) = player.seek(*value)
            {
                this.state.error = Some(e.to_string());
            }
            cx.notify();
        }));
        let settings = Settings {
            theme: crate::preferences::theme::current(cx),
        };
        appearance::apply(settings.theme.is_light(), window, cx);
        let (service, inference, error) = match inference {
            Ok(inference) => match Service::start(inference.clone()) {
                Ok(service) => (Some(service), Some(inference), None),
                Err(error) => (None, Some(inference), Some(format!("{error:#}"))),
            },
            Err(error) => (None, None, Some(error)),
        };
        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |_, cx| {
            weak.update(cx, |this, cx| {
                if this.service.is_none() {
                    return true;
                }
                this.close();
                cx.notify();
                false
            })
            .unwrap_or(true)
        });
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                if this
                    .update_in(cx, |this, window, cx| this.poll(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let line_scroll = ScrollHandle::new();
        let mut studio = Self {
            service,
            inference,
            state: WorkspaceState {
                error,
                ..Default::default()
            },
            fields,
            line_form,
            seek,
            subscriptions,
            line_wheel: super::SmoothScroll::new(line_scroll.clone()),
            line_scroll,
            line_drag_scroll: Default::default(),
            chara_list_wheel: Default::default(),
            chara_form_wheel: Default::default(),
            icons: Default::default(),
            model_seen: None,
            settings,
            playback: None,
            audio_id: None,
            sync: false,
            focus: cx.focus_handle(),
            menu_focus: cx.focus_handle(),
            menu_trigger_focus: cx.focus_handle(),
            file_trigger_focus: cx.focus_handle(),
            menu_index: 0,
            modal: None,
            model_picker_open: false,
            model_error: None,
            model_details_open: false,
            model_trigger_focus: cx.focus_handle(),
            export_trigger_focus: cx.focus_handle(),
            export_naming: crate::project::ExportNaming::Number,
        };
        if let Some(path) = path {
            studio.send(Command::Open {
                path,
                create: false,
            });
        }
        studio.focus.focus(window, cx);
        studio
    }
    pub(super) fn changed(&mut self, field: Field, value: String) {
        let save_line = self.state.change(field, value);
        if matches!(field, Field::Name | Field::Description) {
            self.autosave_chara();
        } else if save_line {
            self.autosave();
        }
    }
    pub(super) fn sync_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync = true;
        let line = self
            .state
            .snapshot
            .editor
            .lines
            .iter()
            .find(|r| Some(&r.id) == self.state.selected.as_ref());
        let text = line.map(|r| r.text.clone()).unwrap_or_default();
        let prompt = line.and_then(|r| r.prompt.clone()).unwrap_or_default();
        self.fields
            .text
            .update(cx, |f, cx| f.set_value(text, window, cx));
        self.fields
            .prompt
            .update(cx, |f, cx| f.set_value(prompt, window, cx));
        self.line_form.set(&self.state.settings_value(), window, cx);
        if let Some(chara) = self.state.active_chara() {
            self.fields
                .name
                .update(cx, |f, cx| f.set_value(chara.name.clone(), window, cx));
            self.fields.description.update(cx, |f, cx| {
                f.set_value(chara.description.clone(), window, cx)
            });
            self.fields.color.update(cx, |f, cx| {
                f.set_value(
                    appearance::chara_color(Some(chara.color.as_str()), rgb(0x41a2ec).into()),
                    window,
                    cx,
                )
            });
        }
        self.sync = false;
    }
}

impl GenerationForm {
    fn set(&self, settings: &GenerationSettings, window: &mut Window, cx: &mut App) {
        self.seed
            .update(cx, |f, cx| f.set_value(settings.seed.clone(), window, cx));
        self.steps.update(cx, |f, cx| {
            f.set_value(settings.num_steps.clone(), window, cx)
        });
        for ((_, slider), value) in self.sliders().into_iter().zip([
            settings.speed().log2() as f32,
            settings.cfg_text,
            settings.cfg_speaker,
            settings.cfg_caption,
        ]) {
            slider.update(cx, |s, cx| s.set_value(value, window, cx));
        }
    }
}

fn make_form(window: &mut Window, cx: &mut Context<Studio>) -> GenerationForm {
    let guidance = |default: f32, cx: &mut Context<Studio>| {
        cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(10.0)
                .step(0.5)
                .default_value(default)
        })
    };
    GenerationForm {
        seed: cx.new(|cx| InputState::new(window, cx).placeholder("ランダム")),
        steps: cx.new(|cx| InputState::new(window, cx).placeholder("モデル既定（40）")),
        speed: cx.new(|_| {
            SliderState::new()
                // Equal distances represent reciprocal speed ratios around 1×.
                .min(-1.0)
                .max(1.0)
                .step(0.01)
                .default_value(0.0)
        }),
        text: guidance(3.0, cx),
        speaker: guidance(5.0, cx),
        caption: guidance(3.0, cx),
        expanded: false,
    }
}
