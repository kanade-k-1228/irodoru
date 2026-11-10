use crate::request::TTSJob;

use std::time::Instant;

use anyhow::{Context, Result};

use tracing::info;

use crate::backend::Runtime;
use crate::cache::voices::Voices;
use crate::config::EngineConfig;
use crate::error::AppError;
use crate::model::{Codec, SynthOutput, TTSModel};
use crate::output::{GenerationConditions, sha256};

pub struct Engine {
    codec: Codec,
    model: TTSModel,
    voices: Voices,
    providers: crate::ProviderTracker,
}

impl Engine {
    pub fn provider_tracker(&self) -> crate::ProviderTracker {
        self.providers.clone()
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
        let model = TTSModel::load(model_dir, &runtime)
            .with_context(|| format!("loading model from {}", model_dir.display()))?;
        let name = &model.manifest.name;
        info!(model = %name, checkpoint = %model.manifest.checkpoint, flow = %model.manifest.flow, secs = t0.elapsed().as_secs_f32(), "model ready");
        Ok(Self {
            codec,
            model,
            voices: Voices::new(),
            providers: runtime.provider_tracker(),
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
        let reference = crate::cache::voices::combine_references(
            job.reference_audio
                .iter()
                .chain(job.reference_audios.iter())
                .map(Vec::as_slice),
            model.manifest.max_ref_seconds,
            self.codec.sample_rate,
        )
        .map_err(AppError::invalid)?;
        if let Some(wav) = &reference {
            params.speaker = Some(self.voices.speaker(wav, model, &mut self.codec)?);
        }
        if is_cancelled() {
            return Err(AppError::internal("synthesis cancelled"));
        }
        model.validate(&params).map_err(AppError::invalid)?;
        let out = model
            .synthesize(&mut self.codec, &params, is_cancelled)
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
        conditions.reference_sha256 = reference.as_deref().map(sha256);
        Ok((conditions, out))
    }
}
