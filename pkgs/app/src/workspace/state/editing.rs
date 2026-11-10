//! Pure editing transitions; the view decides when to synchronize controls and save.
use super::{Field, WorkspaceState};
use crate::domain::{CharaInput, DEFAULT_CHARA_COLOR, GenerationSettings};
use crate::workspace::{editor, protocol::Command};
use anyhow::Result;

impl WorkspaceState {
    pub fn change(&mut self, field: Field, value: String) -> bool {
        if self.busy || (self.chara_locked && matches!(field, Field::Name | Field::Description)) {
            return false;
        }
        match field {
            Field::Text => {
                if let Some(line) = self.selected_line_mut() {
                    line.text = value;
                }
            }
            Field::Description => {
                if let Some(chara) = self.active_chara_mut() {
                    chara.description = value;
                }
                return false;
            }
            Field::Prompt => {
                if let Some(line) = self.selected_line_mut() {
                    line.prompt = Some(value);
                }
            }
            Field::Seed | Field::Steps => {
                let Some(settings) = self.generation_settings_mut() else {
                    return false;
                };
                if matches!(field, Field::Seed) {
                    settings.seed = value;
                } else {
                    settings.num_steps = value;
                }
            }
            Field::Name => {
                if let Some(chara) = self.active_chara_mut() {
                    chara.name = value;
                }
                return false;
            }
        }
        true
    }
    pub fn select(&mut self, id: String) -> bool {
        if self.busy {
            return false;
        }
        if self.selected.as_ref() != Some(&id) {
            self.expanded_line = None;
        }
        self.selected = Some(id);
        if let Some(line) = self.selected_line_mut() {
            let c = line.chara_id.clone();
            self.snapshot.editor.active_chara = c;
        }
        true
    }
    pub fn toggle_line_details(&mut self, id: String) -> bool {
        if self.busy || !self.snapshot.editor.lines.iter().any(|line| line.id == id) {
            return false;
        }
        let was_open = self.expanded_line.as_ref() == Some(&id);
        self.select(id.clone());
        self.expanded_line = (!was_open).then_some(id);
        true
    }
    pub fn select_take(&mut self, id: &str, checked: bool) -> bool {
        if self.busy || self.status.id.is_none() {
            return false;
        }
        let Some(take) = self
            .snapshot
            .takes
            .iter()
            .find(|take| take.id == id)
            .cloned()
        else {
            return false;
        };
        let Some(line) = self.selected_line_mut() else {
            return false;
        };
        if !line.takes.contains(&take.id) {
            return false;
        }
        if checked {
            take.restore_line(line);
        } else if line.take_id.as_deref() == Some(id) {
            line.take_id = None;
        } else {
            return false;
        }
        true
    }

    pub fn create_chara(&self) -> Option<Command> {
        if self.busy || self.status.id.is_none() {
            return None;
        }
        let name = (1..)
            .map(|i| format!("キャラクター {i}"))
            .find(|name| !self.snapshot.charas.iter().any(|c| &c.name == name))?;
        Some(Command::SaveChara {
            id: None,
            input: CharaInput {
                name,
                description: String::new(),
                voice_ids: Vec::new(),
                icon: None,
                color: Some(DEFAULT_CHARA_COLOR.into()),
            },
        })
    }
}

#[derive(Clone, Copy)]
pub enum GenerationField {
    Speed,
    TextGuidance,
    SpeakerGuidance,
    CaptionGuidance,
}

#[derive(Clone, Copy)]
pub enum LineAction {
    Duplicate,
    Delete,
}

impl WorkspaceState {
    fn can_edit_lines(&self) -> bool {
        !self.busy
    }

    pub fn edit_line(&mut self, id: &str, action: LineAction) -> bool {
        if !self.can_edit_lines() || !self.snapshot.editor.lines.iter().any(|line| line.id == id) {
            return false;
        }
        self.select(id.to_owned());
        match action {
            LineAction::Duplicate => self.add_line(true),
            LineAction::Delete => self.remove_line(),
        }
    }

    pub fn add_line(&mut self, duplicate: bool) -> bool {
        if !self.can_edit_lines() {
            return false;
        }
        if self.snapshot.editor.active_chara.is_empty() {
            self.snapshot.editor.active_chara = self
                .snapshot
                .charas
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

    pub fn select_chara(&mut self, id: String) -> bool {
        if !self.can_edit_lines()
            || self.status.id.is_none()
            || !self.snapshot.charas.iter().any(|chara| chara.id == id)
        {
            return false;
        }
        self.snapshot.editor.active_chara = id;
        self.selected = None;
        self.expanded_line = None;
        true
    }

    pub fn append_chara_line(&mut self, chara: String) -> bool {
        if !self.can_edit_lines() {
            return false;
        }
        self.snapshot.editor.active_chara = chara;
        let id = editor::insert(&mut self.snapshot.editor, None, false);
        self.select(id)
    }

    pub fn remove_line(&mut self) -> bool {
        if !self.can_edit_lines() {
            return false;
        }
        let Some(id) = self.selected.clone() else {
            return false;
        };
        let Some(next) = editor::remove(&mut self.snapshot.editor, &id) else {
            return false;
        };
        self.selected = next;
        self.expanded_line = None;

        true
    }

    pub fn move_line(&mut self, after: Option<&str>) -> Option<usize> {
        if !self.can_edit_lines() {
            return None;
        }
        editor::move_line(
            &mut self.snapshot.editor.lines,
            self.selected.as_deref()?,
            after,
        )
    }

    pub fn reset_line_settings(&mut self) -> bool {
        if !self.can_edit_lines() {
            return false;
        }
        let Some(line) = self.selected_line_mut() else {
            return false;
        };
        line.settings = None;
        true
    }

    pub fn change_generation(&mut self, field: GenerationField, value: f32) -> bool {
        if self.busy {
            return false;
        }
        let Some(settings) = self.generation_settings_mut() else {
            return false;
        };
        set_generation(settings, field, value);
        true
    }

    pub fn remove_chara_voice(&mut self, voice: &str) -> bool {
        if self.busy || self.chara_locked {
            return false;
        }
        let Some(chara) = self.active_chara_mut() else {
            return false;
        };
        let previous = chara.voice_ids.len();
        chara.voice_ids.retain(|id| id != voice);
        chara.voice_ids.len() != previous
    }
    pub fn set_chara_icon(&mut self, icon: String) {
        if !self.busy
            && !self.chara_locked
            && let Some(chara) = self.active_chara_mut()
        {
            chara.icon = Some(icon);
        }
    }
    pub fn set_chara_color(&mut self, color: String) {
        if !self.busy
            && !self.chara_locked
            && let Some(chara) = self.active_chara_mut()
        {
            chara.color = color;
        }
    }
    pub fn duplicate_chara(&self) -> Option<Command> {
        if self.busy || self.status.id.is_none() {
            return None;
        }
        let source = self
            .snapshot
            .charas
            .iter()
            .find(|c| c.id == self.snapshot.editor.active_chara)?;
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
            .find(|name| !self.snapshot.charas.iter().any(|c| &c.name == name))?;
        Some(Command::SaveChara {
            id: None,
            input: CharaInput {
                name,
                description: source.description.clone(),
                voice_ids: source.voice_ids.clone(),
                icon: source.icon.clone(),
                color: Some(source.color.clone()),
            },
        })
    }

    pub fn save_chara(&self) -> Option<Command> {
        let chara = self.active_chara()?.clone();
        Some(Command::SaveChara {
            id: Some(chara.id.clone()),
            input: chara.into(),
        })
    }
    pub fn randomize_selected_line(&mut self, seed: u32) -> Result<bool> {
        if !self.can_edit_lines() {
            return Ok(false);
        }
        let Some(id) = self.selected.as_deref() else {
            return Ok(false);
        };
        let newest = self
            .snapshot
            .editor
            .lines
            .iter()
            .find(|line| line.id == id)
            .and_then(|line| line.takes.first());
        let previous = self
            .snapshot
            .takes
            .iter()
            .find(|take| Some(&take.id) == newest)
            .map(|take| take.conditions.seed);
        editor::randomize_line(&mut self.snapshot.editor, id, previous, seed)?;
        Ok(true)
    }
    pub fn generate(&self) -> Result<Option<Command>> {
        if !self.can_edit_lines() {
            return Ok(None);
        }
        let lines = self
            .snapshot
            .editor
            .lines
            .iter()
            .filter(|line| Some(&line.id) == self.selected.as_ref())
            .cloned()
            .collect::<Vec<_>>();
        let requests = editor::requests(&lines, &self.snapshot.charas)?;
        Ok((!requests.is_empty()).then_some(Command::Generate(requests)))
    }
}

fn set_generation(settings: &mut GenerationSettings, field: GenerationField, value: f32) {
    match field {
        GenerationField::Speed => settings.set_speed(value.into()),
        GenerationField::TextGuidance => settings.cfg_text = value,
        GenerationField::SpeakerGuidance => settings.cfg_speaker = value,
        GenerationField::CaptionGuidance => settings.cfg_caption = value,
    }
}
