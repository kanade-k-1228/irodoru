//! Pure editing transitions; the view decides when to synchronize controls and save.
use super::{Draft, Field, WorkspaceState};
use crate::domain::Take;
use crate::domain::{Character, CharacterInput, DEFAULT_CHARACTER_COLOR, DEFAULT_SAMPLE_TEXT};
use crate::workspace::{editor, protocol::Command};
use anyhow::Result;

impl WorkspaceState {
    pub fn change(&mut self, field: Field, value: String) -> bool {
        if self.busy || (self.draft.is_some() && matches!(field, Field::Text | Field::Prompt)) {
            return false;
        }
        match field {
            Field::Text => {
                if let Some(row) = self.selected_row_mut() {
                    row.text = value;
                }
            }
            Field::Description => {
                if let Some(d) = &mut self.draft {
                    d.input.description = value;
                }
                return false;
            }
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
                return false;
            }
            Field::Sample => {
                if let Some(d) = &mut self.draft {
                    d.input.sample_text = Some(value);
                }
                return false;
            }
        }
        self.draft.is_none()
    }
    pub fn select(&mut self, id: String) -> bool {
        if self.busy || self.draft.is_some() {
            return false;
        }
        self.selected = Some(id);
        if let Some(row) = self.selected_row_mut() {
            let c = row.character_id.clone();
            self.snapshot.editor.active_character = c;
        }
        true
    }
    pub fn select_take(&mut self, id: &str, checked: bool) -> bool {
        if self.busy || self.draft.is_some() || self.status.id.is_none() {
            return false;
        }
        let Some(take) = self
            .snapshot
            .takes
            .iter()
            .find(|take| take.id == id && Some(&take.row_id) == self.selected.as_ref())
            .cloned()
        else {
            return false;
        };
        let defaults = self.snapshot.editor.preferences.clone();
        let Some(row) = self.selected_row_mut() else {
            return false;
        };
        if checked {
            take.restore_row(row, &defaults);
        } else if row.take_id.as_deref() == Some(id) {
            row.take_id = None;
        } else {
            return false;
        }
        true
    }

    pub fn edit_character(&mut self, character: Option<Character>) -> bool {
        if self.busy || self.draft.is_some() {
            return false;
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
        true
    }
}

#[derive(Clone, Copy)]
pub enum GenerationField {
    Speed,
    TextGuidance,
    SpeakerGuidance,
    CaptionGuidance,
}

impl Draft {
    pub fn is_dirty(&self) -> bool {
        self.preview.is_some() || self.input != self.original
    }
}

impl WorkspaceState {
    fn can_edit_rows(&self) -> bool {
        !self.busy && self.draft.is_none()
    }

    pub fn add_row(&mut self, duplicate: bool) -> bool {
        if !self.can_edit_rows() {
            return false;
        }
        if self.snapshot.editor.active_character.is_empty() {
            self.snapshot.editor.active_character = self
                .snapshot
                .characters
                .first()
                .map(|c| c.id.clone())
                .unwrap_or_default();
        }
        let id = editor::insert(
            &mut self.snapshot.editor,
            self.selected.as_deref(),
            duplicate,
        );
        self.select(id)
    }

    pub fn insert_at(&mut self, after: Option<&str>, duplicate: bool) -> bool {
        if !self.can_edit_rows()
            || after.is_some_and(|id| !self.snapshot.editor.rows.iter().any(|row| row.id == id))
        {
            return false;
        }
        let source = if duplicate {
            let Some(id) = self
                .selected
                .as_deref()
                .filter(|id| self.snapshot.editor.rows.iter().any(|row| row.id == *id))
            else {
                return false;
            };
            Some(id)
        } else {
            after.or_else(|| self.snapshot.editor.rows.first().map(|row| row.id.as_str()))
        }
        .map(str::to_owned);
        let id = editor::insert(&mut self.snapshot.editor, source.as_deref(), duplicate);
        editor::move_row(&mut self.snapshot.editor.rows, &id, after);
        self.select(id)
    }

    pub fn select_character(&mut self, id: String) -> bool {
        if !self.can_edit_rows()
            || self.status.id.is_none()
            || !self
                .snapshot
                .characters
                .iter()
                .any(|character| character.id == id)
        {
            return false;
        }
        self.snapshot.editor.active_character = id;
        self.selected = None;
        true
    }

    pub fn append_character_row(&mut self, character: String) -> bool {
        if !self.can_edit_rows() {
            return false;
        }
        self.snapshot.editor.active_character = character;
        let id = editor::insert(&mut self.snapshot.editor, None, false);
        self.select(id)
    }

    pub fn copy_take(&mut self, take: &Take) -> bool {
        if !self.can_edit_rows() {
            return false;
        }
        let row = editor::row_from_take(take, &self.snapshot.characters);
        let id = row.id.clone();
        self.snapshot.editor.rows.push(row);
        self.select(id)
    }

    pub fn remove_row(&mut self) -> bool {
        if !self.can_edit_rows() {
            return false;
        }
        let Some(id) = self.selected.clone() else {
            return false;
        };
        let Some(next) = editor::remove(&mut self.snapshot.editor, &id) else {
            return false;
        };
        self.selected = Some(next);
        self.snapshot.takes.retain(|take| take.row_id != id);
        true
    }

    pub fn move_row(&mut self, after: Option<&str>) -> Option<usize> {
        if !self.can_edit_rows() {
            return None;
        }
        editor::move_row(
            &mut self.snapshot.editor.rows,
            self.selected.as_deref()?,
            after,
        )
    }

    pub fn reset_row_settings(&mut self) -> bool {
        if !self.can_edit_rows() {
            return false;
        }
        let Some(row) = self.selected_row_mut() else {
            return false;
        };
        row.settings = None;
        true
    }

    pub fn change_generation(&mut self, field: GenerationField, value: f32) -> bool {
        if self.busy {
            return false;
        }
        let settings = self.generation_settings_mut();
        match field {
            GenerationField::Speed => settings.set_speed(value.into()),
            GenerationField::TextGuidance => settings.cfg_text = value,
            GenerationField::SpeakerGuidance => settings.cfg_speaker = value,
            GenerationField::CaptionGuidance => settings.cfg_caption = value,
        }
        self.draft.is_none()
    }

    pub fn set_draft_voice(&mut self, voice: Option<String>) {
        if !self.busy
            && let Some(draft) = &mut self.draft
        {
            draft.input.voice = voice;
        }
    }
    pub fn set_draft_icon(&mut self, icon: String) {
        if !self.busy
            && let Some(draft) = &mut self.draft
        {
            draft.input.icon = Some(icon);
        }
    }
    pub fn set_draft_color(&mut self, color: String) {
        if !self.busy
            && let Some(draft) = &mut self.draft
        {
            draft.input.color = Some(color);
        }
    }
    pub fn discard_character(&mut self) -> Option<Command> {
        if self.busy {
            return None;
        }
        self.draft.take()?.preview.map(Command::RemovePreview)
    }
    pub fn delete_character(&mut self, id: String) -> Command {
        self.saving_draft = self.draft.is_some();
        Command::DeleteCharacter(id)
    }
    pub fn duplicate_character(&self) -> Option<Command> {
        if self.busy || self.draft.is_some() || self.status.id.is_none() {
            return None;
        }
        let source = self
            .snapshot
            .characters
            .iter()
            .find(|c| c.id == self.snapshot.editor.active_character)?;
        let base = source
            .name
            .rsplit_once(" - ")
            .filter(|(_, suffix)| !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()))
            .map_or(source.name.as_str(), |(base, _)| base);
        let name = (1..)
            .map(|index| {
                let suffix = format!(" - {index}");
                let base: String = base.chars().take(64 - suffix.chars().count()).collect();
                format!("{}{suffix}", base.trim_end())
            })
            .find(|name| !self.snapshot.characters.iter().any(|c| &c.name == name))?;
        Some(Command::SaveCharacter {
            id: None,
            input: CharacterInput {
                name,
                description: source.description.clone(),
                voice: source.voice.clone(),
                icon: source.icon.clone(),
                color: Some(source.color.clone()),
                sample_text: Some(source.sample_text.clone()),
            },
            preview: None,
        })
    }

    pub fn save_character(&mut self) -> Option<Command> {
        if self.busy {
            return None;
        }
        let draft = self.draft.as_ref()?;
        self.saving_draft = true;
        Some(Command::SaveCharacter {
            id: draft.id.clone(),
            input: draft.input.clone(),
            preview: draft.preview.clone(),
        })
    }
    pub fn preview(&self) -> Result<Option<Command>> {
        if self.busy {
            return Ok(None);
        }
        self.draft
            .as_ref()
            .map(|d| {
                Ok(Command::Preview {
                    input: editor::speech(
                        d.input.sample_text.clone().unwrap_or_default(),
                        &d.settings,
                    )?,
                    description: d.input.description.clone(),
                    voice: d.input.voice.clone(),
                })
            })
            .transpose()
    }
    pub fn randomize_selected_row(&mut self, seed: u32) -> Result<bool> {
        if !self.can_edit_rows() {
            return Ok(false);
        }
        let Some(id) = self.selected.as_deref() else {
            return Ok(false);
        };
        let previous = self
            .snapshot
            .takes
            .iter()
            .find(|take| take.row_id == id)
            .map(|take| take.seed);
        editor::randomize_row(&mut self.snapshot.editor, id, previous, seed)?;
        Ok(true)
    }
    pub fn generate(&self, all: bool) -> Result<Option<Command>> {
        if !self.can_edit_rows() {
            return Ok(None);
        }
        let rows = self
            .snapshot
            .editor
            .rows
            .iter()
            .filter(|row| all || Some(&row.id) == self.selected.as_ref())
            .cloned()
            .collect::<Vec<_>>();
        let requests = editor::requests(
            &rows,
            &self.snapshot.characters,
            &self.snapshot.editor.preferences,
        )?;
        Ok((!requests.is_empty()).then_some(Command::Generate(requests)))
    }
}
