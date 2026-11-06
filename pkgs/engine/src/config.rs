use crate::DevicePreference;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct CacheConfig {
    pub caption_bytes_per_model: usize,
    pub reference_bytes: usize,
    pub reference_entries: usize,
}
impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            caption_bytes_per_model: 32 * 1024 * 1024,
            reference_bytes: 64 * 1024 * 1024,
            reference_entries: 16,
        }
    }
}
#[derive(Clone)]
pub struct EngineConfig {
    pub model_dir: PathBuf,
    pub codec_dir: PathBuf,
    pub threads: usize,
    pub device: DevicePreference,
    pub openvino_library: Option<PathBuf>,
    pub cache: CacheConfig,
}
impl EngineConfig {
    pub fn anime(models: PathBuf) -> Self {
        Self {
            model_dir: models.join("anime"),
            codec_dir: models.join("codec"),
            threads: std::thread::available_parallelism().map_or(2, |n| n.get().min(4)),
            device: DevicePreference::Cpu,
            openvino_library: None,
            cache: CacheConfig::default(),
        }
    }
}
