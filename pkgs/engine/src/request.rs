use serde::Deserialize;

use crate::error::AppError;
use crate::model::SynthParams;

const MAX_TEXT_CHARS: usize = 1000;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynthesisRequest {
    pub text: String,
    pub reference_audio: Option<Vec<u8>>,
    pub caption: Option<String>,
    pub seed: Option<u64>,
    pub num_steps: Option<usize>,
    pub cfg_scale_text: Option<f32>,
    pub cfg_scale_speaker: Option<f32>,
    pub cfg_scale_caption: Option<f32>,
    pub cfg_min_t: Option<f32>,
    pub cfg_max_t: Option<f32>,
    pub t_schedule: Option<String>,
    pub sway_coeff: Option<f32>,
    pub duration_scale: Option<f64>,
    pub seconds: Option<f64>,
    pub trim_tail: Option<bool>,
}

pub struct TTSJob {
    pub reference_audio: Option<Vec<u8>>,
    pub params: SynthParams,
}

impl TryFrom<SynthesisRequest> for TTSJob {
    type Error = AppError;

    fn try_from(req: SynthesisRequest) -> Result<Self, AppError> {
        if req.text.trim().is_empty() {
            return Err(AppError::invalid("text is empty"));
        }
        if req.text.chars().count() > MAX_TEXT_CHARS {
            return Err(AppError::invalid(format!(
                "text is too long (max {MAX_TEXT_CHARS} chars)"
            )));
        }
        let sway_coeff = match req.t_schedule.as_deref().unwrap_or("linear") {
            "linear" => None,
            "sway" => Some(req.sway_coeff.unwrap_or(-1.0)),
            other => {
                return Err(AppError::invalid(format!("unknown t_schedule {other:?}")));
            }
        };
        let duration_scale = req.duration_scale.unwrap_or(1.0);
        if !duration_scale.is_finite() || duration_scale <= 0.0 || duration_scale > 5.0 {
            return Err(AppError::invalid("duration_scale must be in (0, 5]"));
        }
        if let Some(s) = req.seconds
            && (!s.is_finite() || s <= 0.0)
        {
            return Err(AppError::invalid("seconds must be > 0"));
        }
        Ok(Self {
            reference_audio: req.reference_audio,
            params: SynthParams {
                text: req.text,
                caption: req.caption,
                speaker: None,
                seed: req.seed,
                num_steps: req.num_steps,
                cfg_scale_text: req.cfg_scale_text.unwrap_or(3.0),
                cfg_scale_speaker: req.cfg_scale_speaker.unwrap_or(5.0),
                cfg_scale_caption: req.cfg_scale_caption.unwrap_or(3.0),
                cfg_min_t: req.cfg_min_t.unwrap_or(0.5),
                cfg_max_t: req.cfg_max_t.unwrap_or(1.0),
                sway_coeff,
                duration_scale,
                seconds: req.seconds,
                trim_tail: req.trim_tail.unwrap_or(true),
            },
        })
    }
}
