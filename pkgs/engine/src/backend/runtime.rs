use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use ndarray::ArrayD;
use tracing::{info, warn};

use super::Input;
use super::device::{DeviceInfo, DevicePreference, candidates};
use super::onnx::{OrtGraph, OrtRuntime};
use super::openvino::{OpenVinoGraph, OpenVinoRuntime};

pub struct Runtime {
    ort: OrtRuntime,
    preference: DevicePreference,
    devices: Vec<DeviceInfo>,
    openvino: Option<OpenVinoRuntime>,
}

impl Runtime {
    pub fn discover(
        threads: usize,
        preference: DevicePreference,
        library_path: Option<&Path>,
    ) -> Result<Self> {
        anyhow::ensure!(threads > 0, "threads must be greater than zero");
        let ort = OrtRuntime::new(threads)?;
        let mut devices = vec![DeviceInfo {
            kind: DevicePreference::Cpu,
            engine: "onnxruntime".into(),
            name: "CPU".into(),
            description: "ONNX Runtime CPU".into(),
            index: 0,
        }];
        let mut openvino = None;
        if preference != DevicePreference::Cpu {
            match OpenVinoRuntime::discover(library_path) {
                Ok((runtime, available)) => {
                    devices.extend(available);
                    openvino = Some(runtime);
                }
                Err(error) if preference == DevicePreference::Auto && library_path.is_none() => {
                    info!(error = %format!("{error:#}"), "OpenVINO unavailable; using ONNX Runtime CPU");
                }
                Err(error) => {
                    return Err(error)
                        .context("loading OpenVINO; install its runtime or select --device cpu");
                }
            }
        }
        for device in &devices {
            info!(kind = %device.kind, engine = %device.engine, name = %device.name, description = %device.description, "inference device discovered");
        }
        if matches!(preference, DevicePreference::Gpu | DevicePreference::Npu)
            && candidates(preference, &devices).is_empty()
        {
            bail!(
                "requested device {preference} is unavailable; check the OpenVINO runtime and GPU/NPU drivers, or select --device cpu"
            );
        }
        Ok(Self {
            ort,
            preference,
            devices,
            openvino,
        })
    }

    pub fn devices(&self) -> &[DeviceInfo] {
        &self.devices
    }

    pub(crate) fn load(&self, path: &Path) -> Result<Graph> {
        self.load_with(path, false)
    }

    pub(crate) fn load_spinning(&self, path: &Path) -> Result<Graph> {
        self.load_with(path, true)
    }

    fn load_with(&self, path: &Path, spinning: bool) -> Result<Graph> {
        let cpu = CpuGraph {
            ort: self.ort.clone(),
            path: path.to_owned(),
            spinning,
        };
        let mut errors = Vec::new();
        for device in candidates(self.preference, &self.devices) {
            let accelerator_path = path.with_extension("portable.onnx");
            let load = || -> Result<OpenVinoGraph> {
                anyhow::ensure!(
                    accelerator_path.is_file(),
                    "missing accelerator graph {}; run pkgs/engine/script/portable_onnx.py --model-dir <model-dir>",
                    accelerator_path.display()
                );
                self.openvino
                    .as_ref()
                    .context("OpenVINO unavailable")?
                    .load(&accelerator_path, &device.name)
            };
            match load() {
                Ok(session) => {
                    info!(graph = %path.display(), device = %device.kind, engine = %device.engine, "graph ready");
                    return Ok(Graph {
                        session: Backend::OpenVino(session),
                        fallback: (self.preference == DevicePreference::Auto).then_some(cpu),
                    });
                }
                Err(error) => {
                    warn!(graph = %path.display(), device = %device.kind, error = %format!("{error:#}"), "accelerator graph rejected");
                    errors.push(format!("{}: {error:#}", device.kind));
                }
            }
        }
        if !matches!(
            self.preference,
            DevicePreference::Auto | DevicePreference::Cpu
        ) {
            bail!(
                "cannot load {} on {}: {}",
                path.display(),
                self.preference,
                errors.join("; ")
            );
        }
        let session = cpu.load()?;
        info!(graph = %path.display(), device = "cpu", "graph ready");
        Ok(Graph {
            session: Backend::Ort(session),
            fallback: None,
        })
    }
}

struct CpuGraph {
    ort: OrtRuntime,
    path: PathBuf,
    spinning: bool,
}

impl CpuGraph {
    fn load(&self) -> Result<OrtGraph> {
        self.ort.load(&self.path, self.spinning)
    }
}

enum Backend {
    Ort(OrtGraph),
    OpenVino(OpenVinoGraph),
}

pub(crate) struct Graph {
    session: Backend,
    fallback: Option<CpuGraph>,
}

impl Graph {
    pub fn run<const N: usize>(
        &mut self,
        inputs: &[(&str, Input<'_>)],
        outputs: [&str; N],
    ) -> Result<[ArrayD<f32>; N]> {
        match Self::run_session(&mut self.session, inputs, outputs) {
            Ok(result) => Ok(result),
            Err(error) => {
                let Some(cpu) = self.fallback.take() else {
                    return Err(error);
                };
                warn!(graph = %cpu.path.display(), error = %format!("{error:#}"), "accelerator execution failed; retrying on CPU");
                self.session = Backend::Ort(cpu.load()?);
                Self::run_session(&mut self.session, inputs, outputs)
                    .context("CPU retry after accelerator failure")
            }
        }
    }

    fn run_session<const N: usize>(
        backend: &mut Backend,
        inputs: &[(&str, Input<'_>)],
        outputs: [&str; N],
    ) -> Result<[ArrayD<f32>; N]> {
        match backend {
            Backend::OpenVino(graph) => graph.run(inputs, outputs),
            Backend::Ort(graph) => graph.run(inputs, outputs),
        }
    }
}
