pub use crate::request::TTSJob;

use std::time::Instant;

use anyhow::{Context, Result};

use tracing::info;

pub(crate) use crate::cache::voices::Voices;
use crate::config::EngineConfig;
use crate::error::AppError;
use crate::model::{Codec, Runtime, SynthOutput, TTSModel};
use crate::output::{GenerationConditions, sha256};

pub struct Engine {
    codec: Codec,
    model: TTSModel,
    voices: Voices,
}

impl Engine {
    pub fn clear_cache(&mut self) {
        self.voices.clear();
        self.model.clear_cache();
    }

    pub fn load(config: &EngineConfig) -> Result<Self> {
        let EngineConfig {
            model_dir,
            codec_dir,
            threads,
            ..
        } = config;
        anyhow::ensure!(
            model_dir.join("manifest.json").is_file(),
            "no model found in {}",
            model_dir.display()
        );
        info!(threads, dir = %model_dir.display(), "loading model");
        let runtime =
            Runtime::discover(*threads, config.device, config.openvino_library.as_deref())?;
        let codec = Codec::load(codec_dir, &runtime)?;
        let t0 = Instant::now();
        let mut model = TTSModel::load(model_dir, &runtime)
            .with_context(|| format!("loading model from {}", model_dir.display()))?;
        model.configure_cache(&config.cache);
        let name = &model.manifest.name;
        info!(model = %name, checkpoint = %model.manifest.checkpoint, flow = %model.manifest.flow, secs = t0.elapsed().as_secs_f32(), "model ready");
        let voices = Voices::new(&config.cache);
        Ok(Self {
            codec,
            model,
            voices,
        })
    }

    pub fn synthesize(
        &mut self,
        job: TTSJob,
    ) -> Result<(GenerationConditions, SynthOutput), AppError> {
        self.synthesize_cancellable(job, &|| false)
    }

    /// Checks for cancellation between inference stages.
    pub fn synthesize_cancellable(
        &mut self,
        job: TTSJob,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<(GenerationConditions, SynthOutput), AppError> {
        if is_cancelled() {
            return Err(AppError::internal("synthesis cancelled"));
        }
        let model = &mut self.model;
        let mut params = job.params;
        if let Some(wav) = &job.reference_audio {
            params.speaker = Some(self.voices.speaker(wav, model, &mut self.codec)?);
        }
        if is_cancelled() {
            return Err(AppError::internal("synthesis cancelled"));
        }
        model.validate(&params).map_err(AppError::invalid)?;
        let out = model
            .synthesize_cancellable(&mut self.codec, &params, is_cancelled)
            .map_err(AppError::internal)?;
        let mut conditions = GenerationConditions::from_params(
            model.manifest.name.clone(),
            &params,
            out.seed,
            out.num_steps,
            out.audio.sample_rate,
        );
        conditions.checkpoint = model.manifest.checkpoint.clone();
        conditions.flow = model.manifest.flow.clone();
        conditions.reference_sha256 = job.reference_audio.as_deref().map(sha256);
        Ok((conditions, out))
    }
}
