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
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SynthesisRequest, TTSJob};
    use serde_json::json;

    fn conditions(input: serde_json::Value) -> GenerationConditions {
        let request: SynthesisRequest = serde_json::from_value(input).unwrap();
        let job = TTSJob::try_from(request).unwrap();
        GenerationConditions::from_params("mf".into(), &job.params, 42, 4, 48_000)
    }

    #[test]
    fn resolved_defaults_and_json_round_trip_preserve_filename() {
        let implicit = conditions(json!({"text": "こんにちは"}));
        let explicit = conditions(json!({
            "text": "こんにちは", "seed": 42, "num_steps": 4,
            "cfg_scale_text": 3.0, "cfg_scale_speaker": 5.0, "cfg_scale_caption": 3.0,
            "cfg_min_t": 0.5, "cfg_max_t": 1.0, "t_schedule": "linear",
            "duration_scale": 1.0, "trim_tail": true
        }));
        assert_eq!(implicit.filename().unwrap(), explicit.filename().unwrap());
        let restored: GenerationConditions =
            serde_json::from_slice(&serde_json::to_vec(&implicit).unwrap()).unwrap();
        assert_eq!(implicit.filename().unwrap(), restored.filename().unwrap());
        assert_eq!(implicit.seed, 42);
        assert_eq!(implicit.num_steps, 4);

        for n in 0..1000 {
            let mut precise = implicit.clone();
            precise.duration_scale = f64::from_bits(1.0f64.to_bits() + n * 123456789);
            let restored: GenerationConditions =
                serde_json::from_slice(&serde_json::to_vec(&precise).unwrap()).unwrap();
            assert_eq!(precise.filename().unwrap(), restored.filename().unwrap());
        }
    }

    #[test]
    fn generation_changes_affect_the_hash() {
        let original = conditions(json!({"text": "こんにちは"}));
        let value = serde_json::to_value(&original).unwrap();
        for (key, replacement) in [
            ("text", json!("さようなら")),
            ("caption", json!("小声で\n悲しげに")),
            ("model", json!("rf")),
            ("checkpoint", json!("other-checkpoint")),
            ("flow", json!("rf")),
            ("voice", json!("さくら")),
            ("reference_sha256", json!(sha256(b"replacement voice"))),
            ("seed", json!(43)),
            ("num_steps", json!(8)),
            ("cfg_scale_text", json!(2.0)),
            ("cfg_scale_speaker", json!(2.0)),
            ("cfg_scale_caption", json!(2.0)),
            ("cfg_min_t", json!(0.2)),
            ("cfg_max_t", json!(0.9)),
            ("t_schedule", json!("sway")),
            ("sway_coeff", json!(-1.0)),
            ("duration_scale", json!(1.2)),
            ("seconds", json!(2.0)),
            ("trim_tail", json!(false)),
            ("sample_rate", json!(24_000)),
        ] {
            let mut changed = value.clone();
            changed[key] = replacement;
            let changed: GenerationConditions = serde_json::from_value(changed).unwrap();
            assert_ne!(
                original.filename().unwrap(),
                changed.filename().unwrap(),
                "{key}"
            );
        }
    }

    #[test]
    fn wav_comment_is_exact_hash_input_and_preserves_pcm_with_both_padding_lengths() {
        let audio = Audio::new(vec![0.0, 0.5, -0.5], 48_000);
        let plain = audio.to_wav().unwrap();
        for text in ["こんにちは", "こんにちは!"] {
            let conditions = conditions(json!({"text": text, "caption": "小声で\n悲しげに"}));
            let wav = conditions.to_wav(&audio).unwrap();
            assert_eq!(
                u32::from_le_bytes(wav[4..8].try_into().unwrap()) as usize,
                wav.len() - 8
            );
            assert_eq!(&wav[8..plain.len()], &plain[8..]);
            let list = &wav[plain.len()..];
            assert_eq!(&list[..4], b"LIST");
            assert_eq!(
                u32::from_le_bytes(list[4..8].try_into().unwrap()) as usize,
                list.len() - 8
            );
            assert_eq!(&list[8..16], b"INFOICMT");
            let size = u32::from_le_bytes(list[16..20].try_into().unwrap()) as usize;
            let json = &list[20..20 + size - 1];
            assert_eq!(json, serde_json::to_vec(&conditions).unwrap());
            assert_eq!(list[20 + size - 1], 0);
            assert_eq!(list.len() % 2, 0);
            assert_eq!(
                conditions.filename().unwrap(),
                format!("{}.wav", sha256(json))
            );
            assert_eq!(
                Audio::from_wav(&wav).unwrap().samples,
                Audio::from_wav(&plain).unwrap().samples
            );
        }
    }
}
