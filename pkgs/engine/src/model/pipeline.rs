use crate::backend::{Graph, Input, Runtime};
use crate::{CacheConfig, cache::ConditionCache};
use anyhow::{Context, Result, bail, ensure};
use ndarray::{Array, ArrayD, Axis, IxDyn, Slice};
use serde::Deserialize;
use std::path::Path;

use super::{
    audio::Audio,
    codec::Codec,
    condition::{Condition, valid_mask},
    diagnostics::{DebugDump, Stopwatch},
    sampling::{Sampler, noise},
    text::{TextTokenizer, normalize_text},
};

const MIN_SECONDS: f64 = 0.5;
const MAX_SECONDS: f64 = 30.0;

pub struct SynthParams {
    pub text: String,
    pub caption: Option<String>,
    pub(crate) speaker: Option<Condition>,
    pub seed: Option<u64>,
    pub num_steps: Option<usize>,
    pub cfg_scale_text: f32,
    pub cfg_scale_speaker: f32,
    pub cfg_scale_caption: f32,
    pub cfg_min_t: f32,
    pub cfg_max_t: f32,
    pub sway_coeff: Option<f32>,
    pub duration_scale: f64,
    pub seconds: Option<f64>,
    pub trim_tail: bool,
}

pub struct SynthOutput {
    pub audio: Audio,
    pub seed: u64,
    pub num_steps: usize,
    pub timings: Vec<(&'static str, f64)>,
}

fn flattening_point(latent: &ArrayD<f32>) -> Result<usize> {
    const WINDOW: usize = 20;
    let (t, d) = (latent.shape()[1], latent.shape()[2]);
    let flat = latent.as_slice().context("latent not contiguous")?;
    let mut padded = flat[..t * d].to_vec();
    padded.extend(std::iter::repeat_n(0f32, WINDOW * d));
    for i in 0..t {
        let w = &padded[i * d..(i + WINDOW) * d];
        let n = w.len() as f64;
        let mean = w.iter().map(|&v| v as f64).sum::<f64>() / n;
        let var = w.iter().map(|&v| (v as f64 - mean).powi(2)).sum::<f64>() / n;
        if var.sqrt() < 0.05 && mean.abs() < 0.1 {
            return Ok(i);
        }
    }
    Ok(t)
}

#[derive(Deserialize)]
pub struct Manifest {
    pub name: String,
    pub checkpoint: String,
    pub flow: String,
    pub latent_dim: usize,
    pub speaker_patch_size: usize,
    pub speaker_dim: usize,
    pub caption_dim: usize,
    pub max_text_len: usize,
    pub max_caption_len: usize,
    pub max_ref_seconds: f64,
    pub default_num_steps: usize,
    pub bos_token_id: i64,
}

pub struct TTSModel {
    pub manifest: Manifest,
    tokenizer: TextTokenizer,
    caption_tokenizer: TextTokenizer,
    backbone: Graph,
    text_proj: Graph,
    caption_proj: Graph,
    speaker_encoder: Graph,
    duration: Graph,
    sampler: Sampler,
    caption_cache: ConditionCache<String>,
}

impl TTSModel {
    pub(crate) fn configure_cache(&mut self, config: &CacheConfig) {
        self.caption_cache = ConditionCache::new(config.caption_bytes_per_model, 1024);
    }
    pub(crate) fn clear_cache(&mut self) {
        self.caption_cache.clear();
    }

    pub fn load(dir: &Path, runtime: &Runtime) -> Result<Self> {
        let manifest: Manifest = serde_json::from_str(
            &std::fs::read_to_string(dir.join("manifest.json")).context("manifest.json")?,
        )?;
        let bos = manifest.bos_token_id;
        let graph = |name: &str| runtime.load(&dir.join(format!("{name}.onnx")));
        Ok(Self {
            tokenizer: TextTokenizer::load(dir, "tokenizer.json", Some("forced_splits.json"), bos)?,
            caption_tokenizer: TextTokenizer::load(dir, "caption_tokenizer.json", None, bos)?,
            backbone: graph("backbone")?,
            text_proj: graph("text_proj")?,
            caption_proj: graph("caption_proj")?,
            speaker_encoder: graph("speaker_encoder")?,
            duration: graph("duration")?,
            sampler: Sampler::load(dir, runtime, manifest.flow == "meanflow")?,
            manifest,
            caption_cache: ConditionCache::new(
                CacheConfig::default().caption_bytes_per_model,
                1024,
            ),
        })
    }

    pub fn validate(&self, p: &SynthParams) -> Result<()> {
        ensure!(
            !normalize_text(&p.text).trim().is_empty(),
            "text became empty after normalization"
        );
        ensure!(
            [
                p.cfg_scale_text,
                p.cfg_scale_speaker,
                p.cfg_scale_caption,
                p.cfg_min_t,
                p.cfg_max_t
            ]
            .iter()
            .all(|v| v.is_finite()),
            "CFG settings must be finite"
        );
        self.sampler.schedule(
            p.num_steps
                .unwrap_or(self.manifest.default_num_steps)
                .clamp(1, 200),
            p.sway_coeff,
        )?;
        Ok(())
    }

    pub fn synthesize(
        &mut self,
        codec: &mut Codec,
        p: &SynthParams,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<SynthOutput> {
        ensure!(!is_cancelled(), "synthesis cancelled");
        let mut clock = Stopwatch::start();
        let text = normalize_text(&p.text).trim().to_string();
        if text.is_empty() {
            bail!("text became empty after normalization");
        }
        let num_steps = p
            .num_steps
            .unwrap_or(self.manifest.default_num_steps)
            .clamp(1, 200);
        let seed = p.seed.unwrap_or_else(|| rand::random::<u32>() as u64);

        let conds = self.conditions(&text, p)?;
        clock.lap("encode_conditions");
        ensure!(!is_cancelled(), "synthesis cancelled");
        let (latent_steps, target_samples) = self.length(&conds, p, codec)?;
        clock.lap("predict_duration");
        ensure!(!is_cancelled(), "synthesis cancelled");
        let guidance = self.sampler.guidance(
            conds,
            [p.cfg_scale_text, p.cfg_scale_speaker, p.cfg_scale_caption],
            p.cfg_min_t,
            p.cfg_max_t,
        )?;
        clock.lap("context_kv");

        ensure!(!is_cancelled(), "synthesis cancelled");
        let dump = DebugDump::from_env();
        let mut x = noise(latent_steps, self.manifest.latent_dim, seed)?;
        if let Some(dump) = &dump {
            dump.f32("noise.f32", &x)?;
        }
        let schedule = self.sampler.schedule(num_steps, p.sway_coeff)?;
        for w in schedule.windows(2) {
            ensure!(!is_cancelled(), "synthesis cancelled");
            let (t, t_next) = (w[0], w[1]);
            let v = self.sampler.velocity(&guidance, &x, t, t - t_next)?;
            x = x + v * (t_next - t);
        }
        clock.lap("sample");

        ensure!(!is_cancelled(), "synthesis cancelled");
        if let Some(dump) = &dump {
            dump.f32("latent.f32", &x)?;
            let ids = self.tokenizer.encode(&text, self.manifest.max_text_len)?;
            dump.json(
                "meta.json",
                &serde_json::json!({
                    "text": text, "ids": ids, "latent_steps": latent_steps,
                    "num_steps": num_steps, "seed": seed,
                }),
            )?;
        }
        ensure!(!is_cancelled(), "synthesis cancelled");
        let mut audio = codec.decode(&x)?;
        ensure!(!is_cancelled(), "synthesis cancelled");
        if let Some(dump) = &dump {
            dump.f32("audio.f32", &audio.samples)?;
        }
        let mut keep = target_samples;
        if p.trim_tail {
            let tail = flattening_point(&x)? * codec.hop;
            if tail > 0 {
                keep = keep.min(tail);
            }
        }
        audio.samples.truncate(keep);
        clock.lap("decode");

        Ok(SynthOutput {
            audio,
            seed,
            num_steps,
            timings: clock.laps,
        })
    }

    fn conditions(&mut self, text: &str, p: &SynthParams) -> Result<[Condition; 3]> {
        let text = self.encode_text(text, false)?;
        let caption = match p
            .caption
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty())
        {
            Some(c) => self.caption_condition(c)?,
            None => Condition::empty(self.manifest.caption_dim),
        };
        let speaker = p
            .speaker
            .clone()
            .unwrap_or_else(|| Condition::empty(self.manifest.speaker_dim));
        Ok([text, speaker, caption])
    }

    fn caption_condition(&mut self, caption: &str) -> Result<Condition> {
        let caption = caption.trim();
        if caption.is_empty() {
            return Ok(Condition::empty(self.manifest.caption_dim));
        }
        if let Some(condition) = self.caption_cache.get(&caption.to_owned()) {
            return Ok(condition);
        }
        let condition = self.encode_text(caption, true)?;
        self.caption_cache
            .insert(caption.to_owned(), condition.clone());
        Ok(condition)
    }

    fn encode_text(&mut self, text: &str, caption: bool) -> Result<Condition> {
        let (tok, max_len, proj) = if caption {
            let m = self.manifest.max_caption_len;
            (&self.caption_tokenizer, m, &mut self.caption_proj)
        } else {
            let m = self.manifest.max_text_len;
            (&self.tokenizer, m, &mut self.text_proj)
        };
        let ids = tok.encode(text, max_len)?;
        let mask = valid_mask(ids.len());
        let ids = Array::from_shape_vec(IxDyn(&[1, ids.len()]), ids)?;
        let [hidden] = self.backbone.run(
            &[
                ("input_ids", Input::I64(&ids)),
                ("mask", Input::Bool(&mask)),
            ],
            ["hidden"],
        )?;
        let [state] = proj.run(
            &[
                ("hidden", Input::F32(&hidden)),
                ("mask", Input::Bool(&mask)),
            ],
            ["state"],
        )?;
        Ok(Condition { state, mask })
    }

    pub fn encode_speaker(&mut self, latent: &ArrayD<f32>, codec: &Codec) -> Result<Condition> {
        let mut t = latent.shape()[1];
        if self.manifest.max_ref_seconds > 0.0 {
            let max_steps = ((self.manifest.max_ref_seconds * codec.sample_rate as f64
                / codec.hop as f64)
                .ceil() as usize)
                .max(1);
            t = t.min(max_steps);
        }
        let p = self.manifest.speaker_patch_size;
        let usable = (t / p) * p;
        if usable == 0 {
            bail!("reference audio is too short");
        }
        let lat = latent
            .slice_axis(Axis(1), Slice::from(0..usable))
            .to_owned();
        let [state] = self.speaker_encoder.run(
            &[
                ("ref_latent", Input::F32(&lat)),
                ("ref_mask", Input::Bool(&valid_mask(usable))),
            ],
            ["speaker_state"],
        )?;
        let mask = valid_mask(state.shape()[1]);
        Ok(Condition { state, mask })
    }

    fn length(
        &mut self,
        conds: &[Condition; 3],
        p: &SynthParams,
        codec: &Codec,
    ) -> Result<(usize, usize)> {
        let sr = codec.sample_rate as f64;
        let hop = codec.hop;
        Ok(match p.seconds {
            Some(sec) => {
                let sec = sec.clamp(MIN_SECONDS, MAX_SECONDS);
                let target = ((sec * sr) as usize).max(1);
                (target.div_ceil(hop), target)
            }
            None => {
                let frames = self.predict_frames(conds)? * p.duration_scale;
                let min_frames = ((MIN_SECONDS * sr / hop as f64).ceil() as usize).max(1);
                let max_frames = ((MAX_SECONDS * sr / hop as f64).floor() as usize).max(1);
                let steps =
                    (frames.round_ties_even().max(0.0) as usize).clamp(min_frames, max_frames);
                (steps, steps * hop)
            }
        })
    }

    fn predict_frames(&mut self, [text, spk, cap]: &[Condition; 3]) -> Result<f64> {
        let has_spk = ArrayD::from_elem(IxDyn(&[1]), spk.is_present());
        let has_cap = ArrayD::from_elem(IxDyn(&[1]), cap.is_present());
        let [log_frames] = self.duration.run(
            &[
                ("text_state", Input::F32(&text.state)),
                ("text_mask", Input::Bool(&text.mask)),
                ("speaker_state", Input::F32(&spk.state)),
                ("has_speaker", Input::Bool(&has_spk)),
                ("caption_state", Input::F32(&cap.state)),
                ("caption_mask", Input::Bool(&cap.mask)),
                ("has_caption", Input::Bool(&has_cap)),
            ],
            ["log_frames"],
        )?;
        Ok((log_frames.iter().next().copied().unwrap_or(0.0) as f64).exp_m1())
    }
}
