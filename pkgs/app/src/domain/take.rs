use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct Take {
    pub conditions: irodoru_engine::GenerationConditions,
    pub id: String,
    pub line_id: String,
    pub chara_id: Option<String>,
    pub chara_name: String,
    pub additional_prompt: String,
    pub voice_ids: Vec<String>,

    pub elapsed_ms: u64,
    pub audio_seconds: f64,

    pub created_at: u64,
}

impl Take {
    pub fn restore_line(&self, line: &mut super::Line) {
        let conditions = &self.conditions;
        line.settings = Some(super::GenerationSettings {
            seed: conditions.seed.to_string(),
            num_steps: conditions.num_steps.to_string(),
            duration_scale: conditions.duration_scale,
            cfg_text: conditions.cfg_scale_text,
            cfg_speaker: conditions.cfg_scale_speaker,
            cfg_caption: conditions.cfg_scale_caption,
        });
        line.text = conditions.text.clone();
        line.prompt = Some(self.additional_prompt.clone());
        line.take_id = Some(self.id.clone());
    }
}
