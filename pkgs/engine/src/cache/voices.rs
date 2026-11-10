use crate::{
    cache::ConditionCache,
    error::AppError,
    model::{Audio, Codec, Condition, TTSModel},
};
use sha2::{Digest, Sha256};

pub struct Voices {
    cache: ConditionCache<[u8; 32]>,
}
impl Voices {
    pub(crate) fn new() -> Self {
        Self {
            cache: ConditionCache::new(64 * 1024 * 1024, 16),
        }
    }

    pub fn speaker(
        &mut self,
        wav: &[u8],
        model: &mut TTSModel,
        codec: &mut Codec,
    ) -> Result<Condition, AppError> {
        let key = Sha256::digest(wav).into();
        self.get_or_encode(key, || {
            let audio = Audio::from_wav(wav).map_err(AppError::invalid)?;
            if audio.samples.len() < audio.sample_rate as usize / 2 {
                return Err(AppError::invalid(
                    "reference audio must be at least 0.5 s long",
                ));
            }
            let latent = codec
                .encode_reference(audio, model.manifest.max_ref_seconds)
                .map_err(AppError::internal)?;
            let condition = model
                .encode_speaker(&latent, codec)
                .map_err(AppError::internal)?;
            Ok(condition)
        })
    }
    fn get_or_encode(
        &mut self,
        key: [u8; 32],
        encode: impl FnOnce() -> Result<Condition, AppError>,
    ) -> Result<Condition, AppError> {
        if let Some(condition) = self.cache.get(&key) {
            return Ok(condition);
        }
        let condition = encode()?;
        self.cache.insert(key, condition.clone());
        Ok(condition)
    }
}
/// Share the model's reference duration budget across recordings so later clips
/// are not discarded by the codec's prefix truncation. Keep single-clip behavior.
pub(crate) fn combine_references<'a>(
    recordings: impl Iterator<Item = &'a [u8]>,
    max_seconds: f64,
    sample_rate: u32,
) -> anyhow::Result<Option<Vec<u8>>> {
    let recordings: Vec<_> = recordings.collect();
    if recordings.is_empty() {
        return Ok(None);
    }
    if recordings.len() == 1 {
        return Ok(Some(recordings[0].to_vec()));
    }
    let budget = if max_seconds > 0.0 {
        (max_seconds * sample_rate as f64) as usize
    } else {
        usize::MAX
    };
    let per_clip = budget / recordings.len();
    anyhow::ensure!(per_clip > 0, "too many reference recordings");
    let mut samples = Vec::new();
    for wav in recordings {
        let audio = Audio::from_wav(wav)?;
        anyhow::ensure!(
            audio.seconds() >= 0.5,
            "reference audio must be at least 0.5 s long"
        );
        let mut audio = audio.resample(sample_rate);
        audio.samples.truncate(per_clip);
        audio.normalize_loudness(-16.0)?;
        samples.extend(audio.samples);
    }
    Ok(Some(Audio::new(samples, sample_rate).to_wav()?))
}
