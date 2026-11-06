use crate::playback::Playback;
use base64::Engine as _;
use gpui_kit::component::{
    Disableable, Theme, ThemeMode,
    button::{Button, ButtonVariants},
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    slider::{Slider, SliderEvent, SliderState, SliderValue},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use irodoru_desktop::{
    editor,
    library::{Character, CharacterInput, DEFAULT_CHARACTER_COLOR, DEFAULT_SAMPLE_TEXT},
    project::ProjectStatus,
    service::{Command, Event, Service, Snapshot},
    settings::Settings,
    types::{EditorState, GenerationSettings, InferenceStatus},
};
use std::{collections::HashMap, time::Duration};

#[derive(Clone)]
struct DragRow(String);
impl Render for DragRow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p_2().bg(rgb(0x41a2ec)).child("行を移動")
    }
}
struct Draft {
    id: Option<String>,
    input: CharacterInput,
    preview: Option<String>,
    settings: GenerationSettings,
    original: CharacterInput,
}
#[derive(Clone, Copy)]
enum Field {
    Prompt,
    Seed,
    Steps,
    Name,
    Sample,
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
pub struct Studio {
    service: Option<Service>,
    snapshot: Snapshot,
    status: ProjectStatus,
    selected: Option<String>,
    draft: Option<Draft>,
    fields: Fields,
    sliders: Vec<Entity<SliderState>>,
    seek: Entity<SliderState>,
    subscriptions: Vec<Subscription>,
    busy: bool,
    saving_draft: bool,
    defaults: bool,
    advanced: bool,
    script_scroll: ScrollHandle,
    error: Option<String>,
    row_errors: HashMap<String, String>,
    running: Option<String>,
    stopping: bool,
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
impl Studio {
    pub fn new(
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
                    if matches!(event, InputEvent::Change) && !this.sync && !this.busy {
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
                    && !this.busy
                    && let Some(draft) = &mut this.draft
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
                if matches!(event, InputEvent::Change) && !this.sync && !this.busy {
                    let value = input.read(cx).value().to_string();
                    if let Some(row) = this.selected_row_mut() {
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
                    && !this.busy
                    && this.draft.is_none()
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
                    && !this.busy
                    && let Some(draft) = &mut this.draft
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
                        && !this.busy
                    {
                        let settings = this.generation_settings_mut();
                        match index {
                            0 => settings.duration_scale = *value as f64,
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
                this.error = Some(e.to_string());
            }
            cx.notify();
        }));
        subscriptions.push(
            cx.subscribe_in(&fields.sample, window, |this, input, event, _, cx| {
                if matches!(event, InputEvent::Change) && !this.sync && !this.busy {
                    this.changed(Field::Sample, input.read(cx).value().to_string());
                    cx.notify();
                }
            }),
        );
        let settings = Settings {
            light: cx.global::<crate::AppTheme>().0,
        };
        Theme::change(
            if settings.light {
                ThemeMode::Light
            } else {
                ThemeMode::Dark
            },
            Some(window),
            cx,
        );
        appearance::apply(settings.light, cx);
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
            snapshot: Snapshot {
                character_previews: Default::default(),
                editor: EditorState::default(),
                characters: Vec::new(),
                takes: Vec::new(),
                voices: Vec::new(),
            },
            status: ProjectStatus {
                id: None,
                name: None,
                path: None,
                state: "closed".into(),
                error: None,
            },
            selected: None,
            draft: None,
            fields,
            sliders,
            seek,
            subscriptions,
            busy: false,
            saving_draft: false,
            defaults: false,
            advanced: false,
            script_scroll: ScrollHandle::new(),
            error,
            row_errors: HashMap::new(),
            running: None,
            stopping: false,
            settings,
            playback: None,
            audio_id: None,
            sync: false,
            focus: cx.focus_handle(),
            menu_focus: cx.focus_handle(),
            menu_trigger_focus: cx.focus_handle(),
            menu_open: false,
            menu_index: 0,
            dialog_open: false,
        };
        if let Some(path) = path {
            studio.send(
                Command::Open {
                    path,
                    create: false,
                },
                true,
            );
        }
        studio.focus.focus(window, cx);
        studio
    }
    fn selected_row_mut(&mut self) -> Option<&mut irodoru_desktop::types::ScriptRow> {
        let id = self.selected.as_ref()?;
        self.snapshot.editor.rows.iter_mut().find(|r| &r.id == id)
    }
    fn settings_value(&self) -> GenerationSettings {
        if let Some(draft) = &self.draft {
            return draft.settings.clone();
        }
        if !self.defaults
            && let Some(row) = self
                .snapshot
                .editor
                .rows
                .iter()
                .find(|r| Some(&r.id) == self.selected.as_ref())
        {
            return row
                .settings
                .clone()
                .unwrap_or_else(|| self.snapshot.editor.preferences.clone());
        }
        self.snapshot.editor.preferences.clone()
    }
    fn generation_settings_mut(&mut self) -> &mut GenerationSettings {
        if let Some(draft) = &mut self.draft {
            return &mut draft.settings;
        }
        if !self.defaults
            && let Some(index) = self
                .snapshot
                .editor
                .rows
                .iter()
                .position(|r| Some(&r.id) == self.selected.as_ref())
        {
            let defaults = self.snapshot.editor.preferences.clone();
            return self.snapshot.editor.rows[index]
                .settings
                .get_or_insert(defaults);
        }
        &mut self.snapshot.editor.preferences
    }
    fn changed(&mut self, field: Field, value: String) {
        match field {
            Field::Prompt => {
                if let Some(row) = self.selected_row_mut() {
                    row.prompt = Some(value);
                }
            }
            Field::Seed => self.generation_settings_mut().seed = value,
            Field::Steps => self.generation_settings_mut().num_steps = value,
            Field::Name => {
                if let Some(d) = &mut self.draft {
                    d.input.name = value;
                }
                return;
            }
            Field::Sample => {
                if let Some(d) = &mut self.draft {
                    d.input.sample_text = Some(value);
                }
                return;
            }
        }
        self.autosave();
    }
    fn send(&mut self, command: Command, busy: bool) {
        if let Some(service) = &self.service {
            match service.send(command) {
                Ok(()) => {
                    if busy {
                        self.busy = true;
                        self.error = None;
                    }
                }
                Err(e) => self.error = Some(e.to_string()),
            }
        }
    }
    fn autosave(&mut self) {
        if self.draft.is_some() {
            return;
        }
        if self.status.id.is_some() {
            self.status.state = "saving".into();
            self.send(Command::SaveEditor(self.snapshot.editor.clone()), false);
        }
    }
    fn sync_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync = true;
        let row = self
            .snapshot
            .editor
            .rows
            .iter()
            .find(|r| Some(&r.id) == self.selected.as_ref());
        let text = row.map(|r| r.text.clone()).unwrap_or_default();
        let prompt = row.and_then(|r| r.prompt.clone()).unwrap_or_default();
        self.fields
            .text
            .update(cx, |f, cx| f.set_value(text, window, cx));
        self.fields
            .prompt
            .update(cx, |f, cx| f.set_value(prompt, window, cx));
        let s = self.settings_value();
        self.fields
            .seed
            .update(cx, |f, cx| f.set_value(s.seed, window, cx));
        self.fields
            .steps
            .update(cx, |f, cx| f.set_value(s.num_steps, window, cx));
        for (slider, value) in self.sliders.iter().zip([
            s.duration_scale as f32,
            s.cfg_text,
            s.cfg_speaker,
            s.cfg_caption,
        ]) {
            slider.update(cx, |s, cx| s.set_value(value, window, cx));
        }
        if let Some(d) = &self.draft {
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
    fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.light = cx.global::<crate::AppTheme>().0;
        if let Some(error) = cx.global::<crate::ThemeWriter>().0.errors.try_iter().last() {
            self.error = Some(error);
        }
        let events = self
            .service
            .as_ref()
            .map(|s| s.events.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for event in events {
            match event {
                Event::Snapshot(snapshot) => {
                    self.snapshot = snapshot;
                    if self.saving_draft {
                        self.saving_draft = false;
                        self.draft = None;
                    }
                    if !self
                        .snapshot
                        .editor
                        .rows
                        .iter()
                        .any(|r| Some(&r.id) == self.selected.as_ref())
                    {
                        self.selected = self.snapshot.editor.rows.first().map(|r| r.id.clone());
                    }
                    self.sync_fields(window, cx);
                }
                Event::Status(status) => {
                    if status.id == self.status.id || self.busy {
                        if let Some(e) = &status.error {
                            self.error = Some(format!("保存できませんでした: {e}"));
                        }
                        self.status = status;
                    }
                }
                Event::Done => {
                    self.busy = false;
                    self.running = None;
                    self.stopping = false;
                }
                Event::Error(e) => {
                    self.error = Some(e);
                    self.saving_draft = false;
                }
                Event::RowStarted(id) => {
                    self.row_errors.remove(&id);
                    self.running = Some(id);
                }
                Event::RowFailed(id, e) => {
                    self.row_errors.insert(id, e);
                }
                Event::Preview(id) => {
                    if let Some(d) = &mut self.draft
                        && let Some(old) = d.preview.replace(id)
                    {
                        self.send(Command::RemovePreview(old), false);
                    }
                }
                Event::Voice(voice) => {
                    if let Some(d) = &mut self.draft {
                        d.input.voice = Some(voice);
                    }
                }
                Event::Icon(icon) => {
                    if let Some(d) = &mut self.draft {
                        d.input.icon = Some(icon);
                    }
                }
                Event::Audio(bytes) => match Playback::new(bytes) {
                    Ok(p) => self.playback = Some(p),
                    Err(e) => {
                        self.error = Some(format!("音声を再生できません: {e}"));
                        self.audio_id = None;
                    }
                },
                Event::Close => {
                    window.remove_window();
                    return;
                }
            }
        }
        if let Some(p) = &self.playback {
            if let Some(error) = p.take_error() {
                self.error = Some(error);
                self.audio_id = None;
            }
            let fraction = if p.duration > 0.0 {
                p.position() / p.duration
            } else {
                0.0
            };
            self.seek
                .update(cx, |s, cx| s.set_value(fraction, window, cx));
        }
        cx.notify();
    }
    pub(super) fn close(&mut self) {
        if self.busy {
            self.error = Some("処理の完了後に閉じてください。".into());
            return;
        }
        if self.draft.is_some() {
            self.error =
                Some("キャラクターの編集を保存またはキャンセルしてから閉じてください。".into());
            return;
        }
        self.autosave();
        self.send(Command::Flush { close: true }, true);
    }
    fn select(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.draft.is_some() {
            return;
        }
        self.selected = Some(id);
        if let Some(row) = self.selected_row_mut() {
            let c = row.character_id.clone();
            self.snapshot.editor.active_character = c;
        }
        self.defaults = false;
        self.autosave();
        self.sync_fields(window, cx);
        cx.notify();
    }
    fn add(&mut self, duplicate: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.draft.is_some() {
            return;
        }
        if self.snapshot.editor.active_character.is_empty() {
            self.snapshot.editor.active_character = self
                .snapshot
                .characters
                .first()
                .map(|c| c.id.clone())
                .unwrap_or_default();
        }
        match editor::insert(
            &mut self.snapshot.editor,
            self.selected.as_deref(),
            duplicate,
        ) {
            Ok(id) => self.select(id, window, cx),
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    fn generate(&mut self, all: bool, random: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.draft.is_some() {
            return;
        }
        if !all {
            self.defaults = false;
        }
        if random && let Some(id) = self.selected.clone() {
            let previous = self
                .snapshot
                .takes
                .iter()
                .find(|t| t.row_id == id)
                .map(|t| t.seed);
            if let Err(error) =
                editor::randomize_row(&mut self.snapshot.editor, &id, previous, rand::random())
            {
                self.error = Some(error.to_string());
                return;
            }
            self.autosave();
            self.sync_fields(window, cx);
        }

        let rows = if all {
            self.snapshot.editor.rows.clone()
        } else {
            self.snapshot
                .editor
                .rows
                .iter()
                .filter(|r| Some(&r.id) == self.selected.as_ref())
                .cloned()
                .collect()
        };
        match editor::requests(
            &rows,
            &self.snapshot.characters,
            &self.snapshot.editor.preferences,
        ) {
            Ok(requests) if !requests.is_empty() => {
                self.autosave();
                self.send(Command::Generate(requests), true);
            }
            Ok(_) => {}
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    fn edit_character(
        &mut self,
        character: Option<Character>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.draft.is_some() {
            return;
        }
        let (id, input) = match character {
            Some(c) => (
                Some(c.id),
                CharacterInput {
                    name: c.name,
                    description: c.description,
                    voice: c.voice,
                    icon: c.icon,
                    color: Some(c.color),
                    sample_text: Some(c.sample_text),
                },
            ),
            None => (
                None,
                CharacterInput {
                    name: String::new(),
                    description: String::new(),
                    voice: None,
                    icon: None,
                    color: Some(DEFAULT_CHARACTER_COLOR.into()),
                    sample_text: Some(DEFAULT_SAMPLE_TEXT.into()),
                },
            ),
        };
        let mut settings = self.snapshot.editor.preferences.clone();
        settings.num_steps.clear();
        if settings.seed.is_empty() {
            settings.seed = "1".into();
        }
        self.draft = Some(Draft {
            id,
            original: input.clone(),
            input,
            preview: None,
            settings,
        });
        self.sync_fields(window, cx);
        cx.notify();
    }
    fn preview(&mut self) {
        if let Some(d) = &self.draft {
            match editor::speech(d.input.sample_text.clone().unwrap_or_default(), &d.settings) {
                Ok(input) => self.send(
                    Command::Preview {
                        input,
                        description: d.input.description.clone(),
                        voice: d.input.voice.clone(),
                    },
                    true,
                ),
                Err(e) => self.error = Some(e.to_string()),
            }
        }
    }
    fn play(&mut self, id: String, preview: bool) {
        let key = format!("{preview}:{id}");
        if self.audio_id.as_ref() == Some(&key)
            && let Some(p) = &self.playback
            && p.position() < p.duration
        {
            p.toggle();
            return;
        }
        self.playback = None;
        self.audio_id = Some(key);
        self.send(Command::Audio { id, preview }, true);
    }
    fn choose_project(&mut self, create: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.draft.is_some() {
            return;
        }
        self.busy = true;
        self.autosave();
        cx.spawn_in(window, async move |this, cx| {
            let dialog = rfd::AsyncFileDialog::new().add_filter("irodoru project", &["zip"]);
            let file = if create {
                dialog.set_file_name("project.zip").save_file().await
            } else {
                dialog.pick_file().await
            };
            let _ = this.update_in(cx, |this, _, cx| {
                if let Some(file) = file {
                    this.playback = None;
                    this.audio_id = None;
                    this.row_errors.clear();
                    this.send(
                        Command::Open {
                            path: file.path().to_owned(),
                            create,
                        },
                        true,
                    );
                } else {
                    this.busy = false;
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn import(&mut self, icon: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        cx.spawn_in(window, async move |this, cx| {
            let file = rfd::AsyncFileDialog::new()
                .add_filter(
                    if icon { "画像" } else { "参照音声" },
                    if icon {
                        irodoru_desktop::media::IMAGE_EXTENSIONS
                    } else {
                        irodoru_desktop::media::AUDIO_EXTENSIONS
                    },
                )
                .pick_file()
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                if let Some(file) = file {
                    this.send(
                        if icon {
                            Command::ImportIcon(file.path().to_owned())
                        } else {
                            Command::ImportVoice(file.path().to_owned())
                        },
                        true,
                    );
                } else {
                    this.busy = false;
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn export(&mut self, id: String, preview: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        let filename = self
            .snapshot
            .takes
            .iter()
            .find(|t| t.id == id)
            .and_then(|t| t.filename.clone())
            .unwrap_or_else(|| "sample.wav".into());
        cx.spawn_in(window, async move |this, cx| {
            let file = rfd::AsyncFileDialog::new()
                .add_filter("WAV", &["wav"])
                .set_file_name(filename)
                .save_file()
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                if let Some(file) = file {
                    this.send(
                        Command::Export {
                            id,
                            preview,
                            path: file.path().to_owned(),
                        },
                        true,
                    );
                } else {
                    this.busy = false;
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn export_character(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        cx.spawn_in(window, async move |this, cx| {
            let file = rfd::AsyncFileDialog::new()
                .add_filter("WAV", &["wav"])
                .set_file_name("sample.wav")
                .save_file()
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                if let Some(file) = file {
                    this.send(
                        Command::ExportCharacter {
                            id,
                            path: file.path().to_owned(),
                        },
                        true,
                    );
                } else {
                    this.busy = false;
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn discard_character(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.playback = None;
        self.audio_id = None;
        if let Some(draft) = self.draft.take()
            && let Some(id) = draft.preview
        {
            self.send(Command::RemovePreview(id), false);
        }
        self.sync_fields(window, cx);
        cx.notify();
    }
    fn cancel_character(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let dirty = self.draft.as_ref().is_some_and(|d| {
            d.preview.is_some()
                || serde_json::to_value(&d.input).ok() != serde_json::to_value(&d.original).ok()
        });
        if !dirty {
            self.discard_character(window, cx);
            return;
        }
        self.busy = true;
        cx.spawn_in(window, async move |this, cx| {
            let choice = rfd::AsyncMessageDialog::new()
                .set_title("irodoru")
                .set_description("未保存の変更を破棄しますか？")
                .set_buttons(rfd::MessageButtons::YesNo)
                .show()
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.busy = false;
                if choice == rfd::MessageDialogResult::Yes {
                    this.discard_character(window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn delete_character(
        &mut self,
        id: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        self.busy = true;
        cx.spawn_in(window, async move |this, cx| {
            let choice = rfd::AsyncMessageDialog::new()
                .set_title("irodoru")
                .set_description(format!(
                    "「{name}」を削除しますか？生成済みの音声は残ります。"
                ))
                .set_buttons(rfd::MessageButtons::YesNo)
                .show()
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                if choice == rfd::MessageDialogResult::Yes {
                    this.saving_draft = this.draft.is_some();
                    this.send(Command::DeleteCharacter(id), true);
                } else {
                    this.busy = false;
                }
                cx.notify();
            });
        })
        .detach();
    }
}

mod views;

mod appearance;

mod character_view;

mod app_menu;
