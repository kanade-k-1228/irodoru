use super::{Fields, Studio, appearance};
use crate::preferences::Settings;
use crate::workspace::protocol::Command;
use crate::workspace::service::Service;
use crate::workspace::state::{Field, WorkspaceState};
use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::component::slider::{SliderEvent, SliderState, SliderValue};
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;
use std::time::Duration;

impl Studio {
    pub(crate) fn new(
        path: Option<std::path::PathBuf>,
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
                    .submit_on_enter(true)
            }),
            description: cx
                .new(|cx| TextareaState::new(window, cx).placeholder("声の特徴・話し方")),
            prompt: input("追加プロンプト", window, cx),
            seed: input("ランダム", window, cx),
            steps: input("モデル既定（40）", window, cx),
            name: input("キャラクター名", window, cx),
            model_directory: input("モデルフォルダのパス", window, cx),
            sample: cx.new(|cx| TextareaState::new(window, cx).placeholder("試聴する文章")),
            color: cx.new(|cx| ColorPickerState::new(window, cx).default_value(rgb(0x41a2ec))),
        };
        let sliders = vec![
            cx.new(|_| {
                SliderState::new()
                    .min(0.5)
                    .max(2.0)
                    .step(0.05)
                    .default_value(1.0)
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
        subscriptions.push(
            cx.subscribe_in(&fields.color, window, |this, _, event, _, cx| {
                if let ColorPickerEvent::Change(Some(color)) = event
                    && !this.sync
                    && !this.state.busy
                    && let Some(draft) = &mut this.state.draft
                {
                    let c: Rgba = (*color).into();
                    draft.input.color = Some(format!(
                        "#{:02x}{:02x}{:02x}",
                        (c.r * 255.).round() as u8,
                        (c.g * 255.).round() as u8,
                        (c.b * 255.).round() as u8
                    ));
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
                    if let Some(row) = this.state.selected_row_mut() {
                        row.text = value;
                        this.autosave();
                    }
                    cx.notify();
                }
                if let InputEvent::PressEnter {
                    secondary,
                    shift: false,
                } = event
                    && !this.sync
                    && !this.state.busy
                    && this.state.draft.is_none()
                    && input.update(cx, |state, cx| {
                        state.marked_text_range(window, cx).is_none()
                    })
                {
                    if *secondary {
                        if this.can_generate() && !input.read(cx).value().trim().is_empty() {
                            this.generate(false, false, window, cx);
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
                if matches!(event, InputEvent::Change)
                    && !this.sync
                    && !this.state.busy
                    && let Some(draft) = &mut this.state.draft
                {
                    draft.input.description = input.read(cx).value().to_string();
                    cx.notify();
                }
            },
        ));
        for (index, slider) in sliders.iter().enumerate() {
            subscriptions.push(
                cx.subscribe_in(slider, window, move |this, _, event, _, cx| {
                    if let SliderEvent::Change(SliderValue::Single(value)) = event
                        && !this.sync
                        && !this.state.busy
                    {
                        let settings = this.state.generation_settings_mut();
                        match index {
                            0 => settings.set_speed(f64::from(*value)),
                            1 => settings.cfg_text = *value,
                            2 => settings.cfg_speaker = *value,
                            _ => settings.cfg_caption = *value,
                        };
                        this.autosave();
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
        let (service, error) = match Service::start() {
            Ok(s) => (Some(s), None),
            Err(e) => (None, Some(format!("{e:#}"))),
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
            state: WorkspaceState {
                error,
                ..Default::default()
            },
            fields,
            sliders,
            seek,
            subscriptions,
            advanced: false,
            script_scroll: ScrollHandle::new(),
            settings,
            playback: None,
            audio_id: None,
            sync: false,
            focus: cx.focus_handle(),
            menu_focus: cx.focus_handle(),
            menu_trigger_focus: cx.focus_handle(),
            menu_open: false,
            project_menu: false,
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
        if self.state.change(field, value) {
            self.autosave();
        }
    }
    pub(super) fn sync_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync = true;
        let row = self
            .state
            .snapshot
            .editor
            .rows
            .iter()
            .find(|r| Some(&r.id) == self.state.selected.as_ref());
        let text = row.map(|r| r.text.clone()).unwrap_or_default();
        let prompt = row.and_then(|r| r.prompt.clone()).unwrap_or_default();
        self.fields
            .text
            .update(cx, |f, cx| f.set_value(text, window, cx));
        self.fields
            .prompt
            .update(cx, |f, cx| f.set_value(prompt, window, cx));
        let s = self.state.settings_value();
        let speed = s.speed() as f32;
        self.fields
            .seed
            .update(cx, |f, cx| f.set_value(s.seed, window, cx));
        self.fields
            .steps
            .update(cx, |f, cx| f.set_value(s.num_steps, window, cx));
        for (slider, value) in
            self.sliders
                .iter()
                .zip([speed, s.cfg_text, s.cfg_speaker, s.cfg_caption])
        {
            slider.update(cx, |s, cx| s.set_value(value, window, cx));
        }
        if let Some(d) = &self.state.draft {
            self.fields
                .name
                .update(cx, |f, cx| f.set_value(d.input.name.clone(), window, cx));
            self.fields.description.update(cx, |f, cx| {
                f.set_value(d.input.description.clone(), window, cx)
            });
            self.fields.sample.update(cx, |f, cx| {
                f.set_value(d.input.sample_text.clone().unwrap_or_default(), window, cx)
            });
            self.fields.color.update(cx, |f, cx| {
                f.set_value(
                    appearance::character_color(d.input.color.as_deref(), rgb(0x41a2ec).into()),
                    window,
                    cx,
                )
            });
        }
        self.sync = false;
    }
}
