use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct Take {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conditions: Option<irodoru_engine::GenerationConditions>,
    pub id: String,
    pub row_id: String,
    pub character_id: Option<String>,
    pub character_name: String,
    pub description: String,
    #[serde(default)]
    pub additional_prompt: String,
    pub voice: Option<String>,
    pub text: String,
    pub model: String,
    #[serde(with = "seed_string")]
    pub seed: u64,
    pub num_steps: usize,

    pub elapsed_ms: u64,
    pub audio_seconds: f64,

    pub created_at: u64,
}

impl Take {
    pub fn restore_row(&self, row: &mut super::ScriptRow, defaults: &super::GenerationSettings) {
        let mut settings = row.settings.clone().unwrap_or_else(|| defaults.clone());
        settings.seed = self.seed.to_string();
        settings.num_steps = self.num_steps.to_string();
        if let Some(conditions) = &self.conditions {
            settings.seed = conditions.seed.to_string();
            settings.num_steps = conditions.num_steps.to_string();
            settings.duration_scale = conditions.duration_scale;
            settings.cfg_text = conditions.cfg_scale_text;
            settings.cfg_speaker = conditions.cfg_scale_speaker;
            settings.cfg_caption = conditions.cfg_scale_caption;
        }
        row.settings = Some(settings);
        row.text = self.text.clone();
        row.prompt = Some(self.additional_prompt.clone());
        row.take_id = Some(self.id.clone());
    }
}

mod seed_string {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(seed: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&seed.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}
