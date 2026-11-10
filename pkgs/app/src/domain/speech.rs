use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SynthesisInput {
    pub text: String,

    pub seed: Option<u64>,
    pub num_steps: Option<usize>,
    pub duration_scale: f64,
    pub cfg_scale_text: f32,
    pub cfg_scale_speaker: f32,
    pub cfg_scale_caption: f32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TakeRequest {
    pub line_id: String,
    pub chara_id: String,
    pub additional_prompt: Option<String>,
    pub request: SynthesisInput,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum InferenceStatus {
    Checking { checked: u64, total: u64 },
    Downloading { downloaded: u64, total: u64 },
    Extracting { extracted: u64, total: u64 },
    Loading,
    Ready,
    Generating,
    Error { error: String },
}
