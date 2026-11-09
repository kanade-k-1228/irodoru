use crate::backend::{Graph, Input, Runtime};
use std::path::Path;

use anyhow::{Result, bail};
use ndarray::{Array, ArrayD, Axis, IxDyn, Slice, concatenate};
use rand::{SeedableRng, rngs::StdRng};
use rand_distr::{Distribution, StandardNormal};

use super::condition::Condition;

const RF_INIT_SCALE: f32 = 0.999;

const KV_NAMES: [&str; 6] = [
    "k_text",
    "v_text",
    "k_speaker",
    "v_speaker",
    "k_caption",
    "v_caption",
];

pub(super) struct Guidance {
    scales: Vec<f32>,
    min_t: f32,
    max_t: f32,
    masks: [ArrayD<bool>; 3],
    ctx: [ArrayD<f32>; 6],
    masks_1: [ArrayD<bool>; 3],
    ctx_1: [ArrayD<f32>; 6],
}

impl Guidance {
    fn rows(&self) -> usize {
        1 + self.scales.len()
    }

    fn is_active(&self, t: f32) -> bool {
        !self.scales.is_empty() && self.min_t <= t && t <= self.max_t
    }

    fn combine(&self, out: &ArrayD<f32>) -> ArrayD<f32> {
        let v0 = out.slice_axis(Axis(0), Slice::from(0..1));
        let mut v = v0.to_owned();
        for (k, s) in self.scales.iter().enumerate() {
            let vk = out.slice_axis(Axis(0), Slice::from(k + 1..k + 2));
            v = v + (&v0 - &vk) * *s;
        }
        v
    }
}

pub(super) struct Sampler {
    meanflow: bool,
    context: Graph,
    step_graph: Graph,
}

impl Sampler {
    pub(super) fn load(dir: &Path, runtime: &Runtime, meanflow: bool) -> Result<Self> {
        Ok(Self {
            meanflow,
            context: runtime.load(&dir.join("dit_context.onnx"))?,
            step_graph: runtime.load_spinning(&dir.join("dit_step.onnx"))?,
        })
    }

    pub(super) fn guidance(
        &mut self,
        conds: [Condition; 3],
        scales: [f32; 3],
        min_t: f32,
        max_t: f32,
    ) -> Result<Guidance> {
        let mut rows = vec![conds];
        let requested_scales = scales;
        let mut scales = Vec::new();
        if !self.meanflow {
            for (k, scale) in requested_scales.into_iter().enumerate() {
                if scale > 0.0 && rows[0][k].is_present() {
                    let mut row = rows[0].clone();
                    row[k] = row[k].dropped();
                    rows.push(row);
                    scales.push(scale);
                }
            }
        }
        let column = |k: usize| Condition::concat(&rows.iter().map(|r| &r[k]).collect::<Vec<_>>());
        let [text, speaker, caption] = [column(0)?, column(1)?, column(2)?];
        let ctx = self.context.run(
            &[
                ("text_state", Input::F32(&text.state)),
                ("speaker_state", Input::F32(&speaker.state)),
                ("caption_state", Input::F32(&caption.state)),
            ],
            KV_NAMES,
        )?;
        Ok(Guidance {
            scales,
            min_t,
            max_t,
            masks_1: rows[0].each_ref().map(|c| c.mask.clone()),
            ctx_1: ctx
                .each_ref()
                .map(|c| c.slice_axis(Axis(1), Slice::from(0..1)).to_owned()),
            masks: [text.mask, speaker.mask, caption.mask],
            ctx,
        })
    }

    pub(super) fn schedule(&self, num_steps: usize, sway: Option<f32>) -> Result<Vec<f32>> {
        let meanflow = self.meanflow;
        let ts: Vec<f32> = (0..=num_steps)
            .map(|i| {
                let u = i as f32 / num_steps as f32;
                if meanflow {
                    return 1.0 - u;
                }
                let u = match sway {
                    Some(c) => {
                        (u + c * ((0.5 * std::f32::consts::PI * u).cos() + u - 1.0)).clamp(0.0, 1.0)
                    }
                    None => u,
                };
                (1.0 - u) * RF_INIT_SCALE
            })
            .collect();
        if ts.iter().any(|t| !t.is_finite()) || ts.windows(2).any(|w| w[0] <= w[1]) {
            bail!("t schedule must be strictly decreasing; adjust num_steps or sway_coeff");
        }
        Ok(ts)
    }

    pub(super) fn velocity(
        &mut self,
        g: &Guidance,
        x: &ArrayD<f32>,
        t: f32,
        delta_t: f32,
    ) -> Result<ArrayD<f32>> {
        if !g.is_active(t) {
            return self.step(x, t, delta_t, &g.masks_1, &g.ctx_1);
        }
        let xb = concatenate(Axis(0), &vec![x.view(); g.rows()])?;
        let out = self.step(&xb, t, delta_t, &g.masks, &g.ctx)?;
        Ok(g.combine(&out))
    }

    fn step(
        &mut self,
        x: &ArrayD<f32>,
        t: f32,
        delta_t: f32,
        masks: &[ArrayD<bool>; 3],
        ctx: &[ArrayD<f32>; 6],
    ) -> Result<ArrayD<f32>> {
        let b = x.shape()[0];
        let t = ArrayD::from_elem(IxDyn(&[b]), t);
        let dt = ArrayD::from_elem(IxDyn(&[b]), delta_t);
        let mut inputs = vec![
            ("x_t", Input::F32(x)),
            ("t", Input::F32(&t)),
            ("delta_t", Input::F32(&dt)),
            ("text_mask", Input::Bool(&masks[0])),
            ("speaker_mask", Input::Bool(&masks[1])),
            ("caption_mask", Input::Bool(&masks[2])),
        ];
        inputs.extend(
            KV_NAMES
                .into_iter()
                .zip(ctx)
                .map(|(n, c)| (n, Input::F32(c))),
        );
        let [v] = self.step_graph.run(&inputs, ["v"])?;
        Ok(v)
    }
}

pub(super) fn noise(latent_steps: usize, latent_dim: usize, seed: u64) -> Result<ArrayD<f32>> {
    let ld = latent_dim;
    let mut rng = StdRng::seed_from_u64(seed);
    let noise: Vec<f32> = (0..latent_steps * ld)
        .map(|_| StandardNormal.sample(&mut rng))
        .collect();
    Ok(Array::from_shape_vec(IxDyn(&[1, latent_steps, ld]), noise)?)
}
