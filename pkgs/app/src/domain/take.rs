use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct Take {
    pub conditions: irodoru_engine::GenerationConditions,
    pub id: String,
    #[serde(rename = "script_id")]
    pub row_id: String,
    pub character_id: Option<String>,
    pub character_name: String,
    pub additional_prompt: String,
    pub voice_ids: Vec<String>,

    pub elapsed_ms: u64,
    pub audio_seconds: f64,

    pub created_at: u64,
}

impl Take {
    pub fn restore_row(&self, row: &mut super::ScriptRow) {
        let conditions = &self.conditions;
        row.settings = Some(super::GenerationSettings {
            seed: conditions.seed.to_string(),
            num_steps: conditions.num_steps.to_string(),
            duration_scale: conditions.duration_scale,
            cfg_text: conditions.cfg_scale_text,
            cfg_speaker: conditions.cfg_scale_speaker,
            cfg_caption: conditions.cfg_scale_caption,
        });
        row.text = conditions.text.clone();
        row.prompt = Some(self.additional_prompt.clone());
        row.take_id = Some(self.id.clone());
    }

    #[cfg(test)]
    pub(crate) fn test_conditions() -> irodoru_engine::GenerationConditions {
        let job: irodoru_engine::TTSJob = irodoru_engine::SynthesisRequest {
            text: "こんにちは".into(),
            ..Default::default()
        }
        .try_into()
        .unwrap();
        irodoru_engine::GenerationConditions::from_params("anime".into(), &job.params, 42, 4, 48000)
    }
}
