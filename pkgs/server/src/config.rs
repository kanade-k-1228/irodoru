use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::inference::DevicePreference;

/// Irodori-TTS HTTP server.
#[derive(Parser)]
pub struct Arg {
    #[command(subcommand)]
    pub command: Cmd,
    /// Log filter, e.g. debug or info,irodoru_engine=debug.
    #[arg(
        long,
        global = true,
        display_order = 100,
        value_name = "FILTER",
        default_value = "info,ort=warn"
    )]
    pub log: String,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Load the model and serve the HTTP API.
    Serve(ServeArgs),
    /// List inference devices without loading models.
    Devices(DeviceArgs),
    /// Print the OpenAPI document.
    Openapi,
    /// Exit successfully if a local server accepts connections.
    Ready {
        /// Port the server listens on.
        #[arg(long, default_value_t = 8080)]
        port: u16,
    },
}

#[derive(Args)]
pub struct ServeArgs {
    /// Exported model (mf, small or anime) to load.
    /// The Docker image keeps its model here; mount another one to switch.
    #[arg(long, value_name = "DIR", default_value = "/models/model")]
    pub model: PathBuf,
    /// DACVAE codec shared by all models.
    #[arg(long, value_name = "DIR", default_value = "/models/codec")]
    pub codec: PathBuf,
    /// HTTP port.
    #[arg(long, default_value_t = 8080)]
    pub port: u16,
    /// CPU threads. Defaults to the number of cores.
    #[arg(long, default_value_t = all_cores())]
    pub threads: usize,
    #[command(flatten)]
    pub device: DeviceArgs,
}

#[derive(Args)]
pub struct DeviceArgs {
    /// auto, cpu, gpu or npu.
    /// Auto tries GPU, NPU, then CPU per graph; explicit gpu/npu fails if unavailable.
    #[arg(long, default_value_t = DevicePreference::Auto)]
    pub device: DevicePreference,
    /// OpenVINO C runtime library (default: search .runtime/openvino and next to the binary).
    #[arg(long, value_name = "PATH")]
    pub openvino_library: Option<PathBuf>,
}

fn all_cores() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get())
}

impl ServeArgs {
    pub fn engine(&self) -> irodoru_engine::EngineConfig {
        irodoru_engine::EngineConfig {
            model_dir: self.model.clone(),
            codec_dir: self.codec.clone(),
            threads: self.threads,
            device: self.device.device,
            openvino_library: self.device.openvino_library.clone(),
            cache: Default::default(),
        }
    }
}
