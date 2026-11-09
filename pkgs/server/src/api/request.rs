use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;
use utoipa::ToSchema;

use irodoru_engine::AppError;
use irodoru_engine::{SynthesisRequest, TTSJob};

/// The JSON body. Omitted fields take the defaults applied in `TTSJob::try_from`.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TTSRequest {
    #[schema(min_length = 1, max_length = 1000)]
    text: String,
    /// Base64-encoded reference WAV; omit for no reference.
    reference_audio_base64: Option<String>,
    /// Voice-design caption describing the speaker/style.
    caption: Option<String>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    cfg_scale_text: Option<f32>,
    cfg_scale_speaker: Option<f32>,
    cfg_scale_caption: Option<f32>,
    cfg_min_t: Option<f32>,
    cfg_max_t: Option<f32>,
    /// "linear" (default) or "sway"
    #[schema(value_type = Option<Schedule>)]
    t_schedule: Option<String>,
    sway_coeff: Option<f32>,
    duration_scale: Option<f64>,
    seconds: Option<f64>,
    trim_tail: Option<bool>,
}

impl TryFrom<TTSRequest> for TTSJob {
    type Error = AppError;
    fn try_from(req: TTSRequest) -> Result<Self, AppError> {
        SynthesisRequest {
            text: req.text,
            reference_audios: Vec::new(),
            reference_audio: req
                .reference_audio_base64
                .map(|s| STANDARD.decode(s).map_err(AppError::invalid))
                .transpose()?,
            caption: req.caption,
            seed: req.seed,
            num_steps: req.num_steps,
            cfg_scale_text: req.cfg_scale_text,
            cfg_scale_speaker: req.cfg_scale_speaker,
            cfg_scale_caption: req.cfg_scale_caption,
            cfg_min_t: req.cfg_min_t,
            cfg_max_t: req.cfg_max_t,
            t_schedule: req.t_schedule,
            sway_coeff: req.sway_coeff,
            duration_scale: req.duration_scale,
            seconds: req.seconds,
            trim_tail: req.trim_tail,
        }
        .try_into()
    }
}
#[allow(dead_code)]
#[derive(ToSchema)]
#[serde(rename_all = "lowercase")]
enum Schedule {
    Linear,
    Sway,
}
