use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GenerationSettings {
    pub seed: String,
    pub num_steps: String,
    pub cfg_text: f32,
    pub cfg_speaker: f32,
    pub cfg_caption: f32,
    pub duration_scale: f64,
}
impl Default for GenerationSettings {
    fn default() -> Self {
        Self {
            seed: String::new(),
            num_steps: String::new(),
            cfg_text: 3.0,
            cfg_speaker: 5.0,
            cfg_caption: 3.0,
            duration_scale: 1.0,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ScriptRow {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub take_id: Option<String>,
    pub id: String,
    pub text: String,
    pub character_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub settings: Option<GenerationSettings>,
}
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct EditorState {
    pub rows: Vec<ScriptRow>,
    pub preferences: GenerationSettings,
    pub active_character: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SynthesisInput {
    pub text: String,
    #[ts(type = "number | null")]
    pub seed: Option<u64>,
    pub num_steps: Option<usize>,
    pub duration_scale: f64,
    pub cfg_scale_text: f32,
    pub cfg_scale_speaker: f32,
    pub cfg_scale_caption: f32,
}
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct TakeRequest {
    pub row_id: String,
    pub character_id: String,
    pub additional_prompt: Option<String>,
    pub request: SynthesisInput,
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum InferenceStatus {
    Loading,
    Checking {
        #[ts(type = "number")]
        checked: u64,
        #[ts(type = "number")]
        total: u64,
    },
    Downloading {
        #[ts(type = "number")]
        downloaded: u64,
        #[ts(type = "number")]
        total: u64,
    },
    Extracting {
        #[ts(type = "number")]
        extracted: u64,
        #[ts(type = "number")]
        total: u64,
    },
    Ready,
    Generating,
    Error {
        error: String,
    },
}
