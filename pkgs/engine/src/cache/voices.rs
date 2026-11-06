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
