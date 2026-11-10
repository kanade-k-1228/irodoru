use super::{Fields, GenerationSliders, Studio, appearance};
use crate::preferences::Settings;
use crate::workspace::protocol::Command;
use crate::workspace::service::Service;
use crate::workspace::state::{Field, GenerationField, WorkspaceState};
use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::component::slider::{SliderEvent, SliderState, SliderValue};
use gpui_kit::component::{Theme, ThemeMode};
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
            seed: input("ランダム", window, cx),
            preview_seed: input("ランダム", window, cx),
            preview_steps: input("モデル既定（40）", window, cx),
            steps: input("モデル既定（40）", window, cx),
            name: input("キャラクター名", window, cx),
            sample: cx.new(|cx| TextareaState::new(window, cx).placeholder("試聴する文章")),
            color: cx.new(|cx| ColorPickerState::new(window, cx).default_value(rgb(0x41a2ec))),
        };
        let sliders = make_sliders(cx);
        let preview_sliders = make_sliders(cx);
        let seek = cx.new(|_| SliderState::new().min(0.0).max(1.0).step(0.001));
        let mut subscriptions = Vec::new();
        for (entity, field) in [
            (&fields.prompt, Field::Prompt),
            (&fields.seed, Field::Seed),
            (&fields.steps, Field::Steps),
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
        for (entity, seed) in [(&fields.preview_seed, true), (&fields.preview_steps, false)] {
            subscriptions.push(cx.subscribe_in(
                entity,
                window,
                move |this, input, event, _, cx| {
                    if matches!(event, InputEvent::Change)
                        && !this.sync
                        && !this.state.busy
                        && !this.state.chara_locked
                    {
                        let value = input.read(cx).value().to_string();
                        if seed {
                            this.state.preview_settings.seed = value;
                        } else {
                            this.state.preview_settings.num_steps = value;
                        }
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
                    secondary,
                    shift: false,
                } = event
                    && !this.sync
                    && !this.state.busy
                    && input.update(cx, |state, cx| {
                        state.marked_text_range(window, cx).is_none()
                    })
                {
                    if *secondary {
                        if this.can_generate() && !input.read(cx).value().trim().is_empty() {
                            this.generate(false, window, cx);
                        }
                    } else {
                        this.add(false, window, cx);
                        input.update(cx, |state, cx| state.focus(window, cx));
                    }
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
        for (field, slider) in sliders.iter() {
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
        for (field, slider) in preview_sliders.iter() {
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
                        this.state.change_preview_generation(field, value);
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
        subscriptions.push(
            cx.subscribe_in(&fields.sample, window, |this, input, event, _, cx| {
                if matches!(event, InputEvent::Change) && !this.sync && !this.state.busy {
                    this.changed(Field::Sample, input.read(cx).value().to_string());
                    cx.notify();
                }
            }),
        );
        let settings = Settings {
            theme: crate::preferences::theme::current(cx),
        };
        Theme::change(
            if settings.theme.is_light() {
                ThemeMode::Light
            } else {
                ThemeMode::Dark
            },
            Some(window),
            cx,
        );
        appearance::apply(settings.theme.is_light(), cx);
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
        let mut studio = Self {
            service,
            inference,
            state: WorkspaceState {
                error,
                ..Default::default()
            },
            fields,
            sliders,
            preview_sliders,
            seek,
            subscriptions,
            advanced: false,
            preview_advanced: false,
            line_scroll: ScrollHandle::new(),
            line_drag_scroll: Default::default(),
            settings,
            playback: None,
            audio_id: None,
            sync: false,
            focus: cx.focus_handle(),
            menu_focus: cx.focus_handle(),
            menu_trigger_focus: cx.focus_handle(),
            menu_open: false,
            file_trigger_focus: cx.focus_handle(),
            menu_index: 0,
            dialog_open: false,
            model_dialog_open: false,
            model_picker_open: false,
            model_error: None,
            model_trigger_focus: cx.focus_handle(),
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
        if matches!(field, Field::Name | Field::Description | Field::Sample) {
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
        let s = self.state.settings_value();
        let speed = s.speed().log2() as f32;
        self.fields
            .seed
            .update(cx, |f, cx| f.set_value(s.seed, window, cx));
        self.fields
            .steps
            .update(cx, |f, cx| f.set_value(s.num_steps, window, cx));
        for ((_, slider), value) in
            self.sliders
                .iter()
                .into_iter()
                .zip([speed, s.cfg_text, s.cfg_speaker, s.cfg_caption])
        {
            slider.update(cx, |s, cx| s.set_value(value, window, cx));
        }
        if let Some(chara) = self.state.active_chara() {
            self.fields
                .name
                .update(cx, |f, cx| f.set_value(chara.name.clone(), window, cx));
            self.fields.description.update(cx, |f, cx| {
                f.set_value(chara.description.clone(), window, cx)
            });
            self.fields.sample.update(cx, |f, cx| {
                f.set_value(chara.sample_text.clone(), window, cx)
            });
            self.fields.color.update(cx, |f, cx| {
                f.set_value(
                    appearance::chara_color(Some(chara.color.as_str()), rgb(0x41a2ec).into()),
                    window,
                    cx,
                )
            });
        }
        let preview = &self.state.preview_settings;
        self.fields
            .preview_seed
            .update(cx, |f, cx| f.set_value(preview.seed.clone(), window, cx));
        self.fields.preview_steps.update(cx, |f, cx| {
            f.set_value(preview.num_steps.clone(), window, cx)
        });
        for ((_, slider), value) in self.preview_sliders.iter().into_iter().zip([
            preview.speed().log2() as f32,
            preview.cfg_text,
            preview.cfg_speaker,
            preview.cfg_caption,
        ]) {
            slider.update(cx, |s, cx| s.set_value(value, window, cx));
        }
        self.sync = false;
    }
}

fn make_sliders(cx: &mut Context<Studio>) -> GenerationSliders {
    let [speed, text, speaker, caption] = [
        cx.new(|_| {
            SliderState::new()
                // Equal distances represent reciprocal speed ratios around 1×.
                .min(-1.0)
                .max(1.0)
                .step(0.01)
                .default_value(0.0)
        }),
        cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(10.0)
                .step(0.5)
                .default_value(3.0)
        }),
        cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(10.0)
                .step(0.5)
                .default_value(5.0)
        }),
        cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(10.0)
                .step(0.5)
                .default_value(3.0)
        }),
    ];
    GenerationSliders {
        speed,
        text,
        speaker,
        caption,
    }
}
