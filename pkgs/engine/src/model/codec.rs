use super::{
    audio::Audio,
    onnx::{Graph, Input, Runtime},
};
use anyhow::Result;
use ndarray::{Array, ArrayD, IxDyn};
use serde::Deserialize;
use std::path::Path;

const REF_NORMALIZE_DB: f64 = -16.0;

#[derive(Deserialize)]
struct CodecInfo {
    sample_rate: u32,
    hop_length: usize,
}

pub struct Codec {
    enc: Graph,
    dec: Graph,
    pub sample_rate: u32,
    pub hop: usize,
}

impl Codec {
    pub fn load(dir: &Path, runtime: &Runtime) -> Result<Self> {
        let info: CodecInfo =
            serde_json::from_str(&std::fs::read_to_string(dir.join("codec.json"))?)?;
        Ok(Self {
            enc: runtime.load(&dir.join("dacvae_encoder.onnx"))?,
            dec: runtime.load(&dir.join("dacvae_decoder.onnx"))?,
            sample_rate: info.sample_rate,
            hop: info.hop_length,
        })
    }

    pub fn encode_reference(
        &mut self,
        mut audio: Audio,
        max_ref_seconds: f64,
    ) -> Result<ArrayD<f32>> {
        if max_ref_seconds > 0.0 {
            audio.truncate_seconds(max_ref_seconds);
        }
        let mut audio = audio.resample(self.sample_rate);
        audio.normalize_loudness(REF_NORMALIZE_DB)?;
        audio.reflect_pad_to_multiple(self.hop)?;
        let x = Array::from_shape_vec(IxDyn(&[1, 1, audio.samples.len()]), audio.samples)?;
        let [latent] = self.enc.run(&[("wav", Input::F32(&x))], ["latent"])?;
        Ok(latent)
    }

    pub fn decode(&mut self, latent: &ArrayD<f32>) -> Result<Audio> {
        let [audio] = self.dec.run(&[("latent", Input::F32(latent))], ["audio"])?;
        Ok(Audio::new(
            audio.into_raw_vec_and_offset().0,
            self.sample_rate,
        ))
    }
}
