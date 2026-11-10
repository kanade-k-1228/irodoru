use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

use anyhow::{Context, Result, bail};
use ndarray::ArrayD;
use tracing::{info, warn};

use super::Input;
use super::device::{DeviceInfo, DevicePreference, candidates};
use super::onnx::{OrtGraph, OrtRuntime};
use super::openvino::{OpenVinoGraph, OpenVinoRuntime};

/// The provider selected for a graph, not per-operator execution telemetry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderStatus {
    pub graph: String,
    pub provider: String,
    pub fallback: bool,
}

#[derive(Clone, Default)]
pub struct ProviderTracker(Arc<Mutex<Vec<Weak<Mutex<ProviderStatus>>>>>);
impl ProviderTracker {
    pub fn snapshot(&self) -> Vec<ProviderStatus> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter_map(Weak::upgrade)
            .map(|status| status.lock().unwrap().clone())
            .collect()
    }

    fn track(&self, path: &Path, provider: String, fallback: bool) -> Arc<Mutex<ProviderStatus>> {
        let status = Arc::new(Mutex::new(ProviderStatus {
            graph: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            provider,
            fallback,
        }));
        let mut graphs = self.0.lock().unwrap();
        graphs.retain(|graph| graph.strong_count() > 0);
        graphs.push(Arc::downgrade(&status));
        status
    }
}

fn cpu_provider() -> String {
    #[cfg(target_os = "windows")]
    return format!("{} / CPU", crate::windows_runtime::name());
    #[cfg(not(target_os = "windows"))]
    "ONNX Runtime / CPU".into()
}

pub struct Runtime {
    ort: OrtRuntime,
    providers: ProviderTracker,
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
            #[cfg(target_os = "windows")]
            if library_path.is_none() {
                match ort.devices() {
                    Ok(available) => devices.extend(available),
                    Err(error) if preference == DevicePreference::Auto => {
                        warn!(error = %format!("{error:#}"), "Windows ML accelerators unavailable; using CPU");
                    }
                    Err(error) => {
                        return Err(error).context("discovering Windows ML GPU/NPU providers");
                    }
                }
            }
            #[cfg(target_os = "macos")]
            if library_path.is_none() {
                use ort::ep::{CoreML, ExecutionProvider};
                let available = match CoreML::is_available() {
                    Ok(available) => available,
                    Err(error) if preference == DevicePreference::Auto => {
                        warn!(%error, "Core ML unavailable; using CPU");
                        false
                    }
                    Err(error) => return Err(error).context("discovering Core ML"),
                };
                if available {
                    // These are allowed compute policies, not enumerated hardware.
                    for (index, kind, name) in [
                        (0, DevicePreference::Gpu, "Core ML (CPU/GPU)"),
                        (1, DevicePreference::Npu, "Core ML (CPU/Neural Engine)"),
                    ] {
                        devices.push(DeviceInfo {
                            kind,
                            index,
                            engine: "coreml".into(),
                            name: name.into(),
                            description:
                                "Core ML compute policy; hardware assignment is managed by macOS"
                                    .into(),
                        });
                    }
                }
            }
            // Preserve explicit OpenVINO overrides for existing integrations.
            if !cfg!(any(target_os = "windows", target_os = "macos")) || library_path.is_some() {
                match OpenVinoRuntime::discover(library_path) {
                    Ok((runtime, available)) => {
                        devices.extend(available);
                        openvino = Some(runtime);
                    }
                    Err(error)
                        if preference == DevicePreference::Auto && library_path.is_none() =>
                    {
                        info!(error = %format!("{error:#}"), "OpenVINO unavailable; using ONNX Runtime CPU");
                    }
                    Err(error) => {
                        return Err(error).context(
                            "loading OpenVINO; install its runtime or select --device cpu",
                        );
                    }
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
                "requested device {preference} is unavailable; check the inference runtime and GPU/NPU drivers, or select --device cpu"
            );
        }
        Ok(Self {
            ort,
            providers: ProviderTracker::default(),
            preference,
            devices,
            openvino,
        })
    }

    pub fn provider_tracker(&self) -> ProviderTracker {
        self.providers.clone()
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
            #[cfg(target_os = "windows")]
            if device.engine == "windows-ml" {
                match self.ort.load_device(path, spinning, device.index) {
                    Ok(session) => {
                        info!(graph = %path.display(), device = %device.kind, provider = %device.name, "Windows ML graph ready");
                        return Ok(Graph {
                            session: Backend::Ort(session),
                            status: self.providers.track(
                                path,
                                format!("Windows ML / {} ({})", device.name, device.kind),
                                false,
                            ),
                            fallback: (self.preference == DevicePreference::Auto).then_some(cpu),
                        });
                    }
                    Err(error) => {
                        warn!(graph = %path.display(), provider = %device.name, error = %format!("{error:#}"), "Windows ML graph rejected");
                        errors.push(format!("{}: {error:#}", device.name));
                        continue;
                    }
                }
            }
            #[cfg(target_os = "macos")]
            if device.engine == "coreml" {
                // Auto tries ALL once; the second entry is only for explicit NPU policy.
                if self.preference == DevicePreference::Auto && device.kind == DevicePreference::Npu
                {
                    continue;
                }
                match self.ort.load_coreml(path, spinning, self.preference) {
                    Ok(session) => {
                        let label = match self.preference {
                            DevicePreference::Gpu => "Core ML（CPU/GPU）",
                            DevicePreference::Npu => "Core ML（CPU/Neural Engine）",
                            _ => "Core ML（自動選択）",
                        };
                        info!(graph = %path.display(), provider = label, "Core ML graph ready");
                        return Ok(Graph {
                            session: Backend::Ort(session),
                            status: self.providers.track(path, label.into(), false),
                            fallback: (self.preference == DevicePreference::Auto).then_some(cpu),
                        });
                    }
                    Err(error) => {
                        warn!(graph = %path.display(), error = %format!("{error:#}"), "Core ML graph rejected");
                        errors.push(format!("Core ML: {error:#}"));
                        continue;
                    }
                }
            }
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
                        status: self.providers.track(
                            path,
                            format!("OpenVINO / {}", device.name),
                            false,
                        ),
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
            status: self.providers.track(
                path,
                cpu_provider(),
                self.preference == DevicePreference::Auto,
            ),
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
    status: Arc<Mutex<ProviderStatus>>,
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
                {
                    let mut status = self.status.lock().unwrap();
                    status.provider = cpu_provider();
                    status.fallback = true;
                }
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
