use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::model::{Audio, SynthParams};

#[derive(Clone, Deserialize, Serialize)]
pub struct GenerationConditions {
    pub schema_version: u32,
    pub model: String,
    pub checkpoint: String,
    pub flow: String,
    pub voice: Option<String>,
    pub reference_sha256: Option<String>,
    pub text: String,
    pub caption: Option<String>,
    pub seed: u64,
    pub num_steps: usize,
    pub cfg_scale_text: f32,
    pub cfg_scale_speaker: f32,
    pub cfg_scale_caption: f32,
    pub cfg_min_t: f32,
    pub cfg_max_t: f32,
    pub t_schedule: String,
    pub sway_coeff: Option<f32>,
    pub duration_scale: f64,
    pub seconds: Option<f64>,
    pub trim_tail: bool,
    pub sample_rate: u32,
}

impl GenerationConditions {
    pub fn from_params(
        model: String,
        p: &SynthParams,
        seed: u64,
        num_steps: usize,
        sample_rate: u32,
    ) -> Self {
        Self {
            schema_version: 1,
            model,
            checkpoint: String::new(),
            flow: String::new(),
            voice: None,
            reference_sha256: None,
            text: p.text.clone(),
            caption: p.caption.clone(),
            seed,
            num_steps,
            cfg_scale_text: p.cfg_scale_text,
            cfg_scale_speaker: p.cfg_scale_speaker,
            cfg_scale_caption: p.cfg_scale_caption,
            cfg_min_t: p.cfg_min_t,
            cfg_max_t: p.cfg_max_t,
            t_schedule: if p.sway_coeff.is_some() {
                "sway"
            } else {
                "linear"
            }
            .into(),
            sway_coeff: p.sway_coeff,
            duration_scale: p.duration_scale,
            seconds: p.seconds,
            trim_tail: p.trim_tail,
            sample_rate,
        }
    }

    pub fn filename(&self) -> Result<String> {
        Ok(format!("{}.wav", sha256(&serde_json::to_vec(self)?)))
    }

    pub fn to_wav(&self, audio: &Audio) -> Result<Vec<u8>> {
        audio.to_wav_with_comment(&serde_json::to_string(self)?)
    }
}

pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
