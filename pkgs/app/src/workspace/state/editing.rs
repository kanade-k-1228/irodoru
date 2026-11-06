//! Pure editing transitions; the view decides when to synchronize controls and save.
use super::{Draft, Field, WorkspaceState};
use crate::domain::{Character, CharacterInput, DEFAULT_CHARACTER_COLOR, DEFAULT_SAMPLE_TEXT};

impl WorkspaceState {
    pub fn change(&mut self, field: Field, value: String) -> bool {
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
                return false;
            }
            Field::Sample => {
                if let Some(d) = &mut self.draft {
                    d.input.sample_text = Some(value);
                }
                return false;
            }
        }
        true
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
        self.defaults = false;
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
