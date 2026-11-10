use crate::DevicePreference;
use std::path::PathBuf;

#[derive(Clone)]
pub struct EngineConfig {
    pub model_dir: PathBuf,
    pub codec_dir: PathBuf,
    pub threads: usize,
    pub device: DevicePreference,
    pub openvino_library: Option<PathBuf>,
}
impl EngineConfig {
    pub fn anime(models: PathBuf) -> Self {
        Self {
            model_dir: models.join("anime"),
            codec_dir: models.join("codec"),
            threads: std::thread::available_parallelism().map_or(2, |n| n.get().min(4)),
            device: DevicePreference::Cpu,
            openvino_library: None,
        }
    }
}
