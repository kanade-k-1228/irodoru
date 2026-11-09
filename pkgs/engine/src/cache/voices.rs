use crate::{CacheConfig, cache::ConditionCache};
use crate::{
    error::AppError,
    model::{Audio, Codec, Condition, TTSModel},
};
use sha2::{Digest, Sha256};

pub struct Voices {
    cache: ConditionCache<[u8; 32]>,
}
impl Default for Voices {
    fn default() -> Self {
        Self::new(&CacheConfig::default())
    }
}
impl Voices {
    pub(crate) fn new(config: &CacheConfig) -> Self {
        Self {
            cache: ConditionCache::new(config.reference_bytes, config.reference_entries),
        }
    }
    pub(crate) fn clear(&mut self) {
        self.cache.clear();
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_is_bounded_refreshes_hits_and_never_caches_failures() {
        let mut cache = Voices::default();
        let key = |i: u8| [i; 32];
        for i in 0..16 {
            cache
                .get_or_encode(key(i), || Ok(Condition::test_value(i as f32)))
                .unwrap();
        }
        let mut hit = cache.get_or_encode(key(0), || panic!("must hit")).unwrap();
        hit.test_fill(99.0);
        cache
            .get_or_encode(key(16), || Ok(Condition::test_value(16.0)))
            .unwrap();
        assert_eq!(cache.cache.entries.len(), 16);
        assert!(!cache.cache.entries.iter().any(|(k, _)| *k == key(1)));
        assert_eq!(
            cache
                .get_or_encode(key(0), || panic!("recent hit retained"))
                .unwrap()
                .test_scalar(),
            0.0
        );
        assert!(
            cache
                .get_or_encode(key(17), || Err(AppError::invalid("encode failed")))
                .is_err()
        );
        assert_eq!(cache.cache.entries.len(), 16);
        assert_eq!(
            cache
                .get_or_encode(key(17), || Ok(Condition::test_value(42.0)))
                .unwrap()
                .test_scalar(),
            42.0
        );
        cache.clear();
        assert_eq!(cache.cache.len(), 0);
    }
}

#[cfg(test)]
mod reference_tests {
    use super::*;

    #[test]
    fn multiple_recordings_share_the_budget_and_keep_every_clip() {
        let first = Audio::new(vec![0.1; 48000], 24000).to_wav().unwrap();
        let second = Audio::new(vec![-0.1; 96000], 48000).to_wav().unwrap();
        let mixed = combine_references(
            [first.as_slice(), second.as_slice()].into_iter(),
            2.0,
            48000,
        )
        .unwrap()
        .unwrap();
        let audio = Audio::from_wav(&mixed).unwrap();
        assert_eq!(audio.sample_rate, 48000);
        assert_eq!(audio.samples.len(), 96000);
        assert!(audio.samples[24000] > 0.0);
        assert!(audio.samples[72000] < 0.0);
        assert_eq!(
            combine_references([first.as_slice()].into_iter(), 2.0, 48000).unwrap(),
            Some(first)
        );
        assert!(
            combine_references(std::iter::empty(), 2.0, 48000)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn an_invalid_or_too_short_clip_rejects_the_whole_reference_set() {
        let valid = Audio::new(vec![0.1; 48000], 48000).to_wav().unwrap();
        let short = Audio::new(vec![0.1; 100], 48000).to_wav().unwrap();
        for invalid in [short.as_slice(), b"not a wav"] {
            assert!(
                combine_references([valid.as_slice(), invalid].into_iter(), 2.0, 48000).is_err()
            );
        }
    }
}
