use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct Take {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
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
