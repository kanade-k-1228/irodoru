use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use ndarray::ArrayD;
use ort::environment::{Environment, GlobalThreadPoolOptions};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::{Session, SessionInputValue};
use ort::value::TensorRef;
use tracing::{info, warn};

use super::device::{DeviceInfo, DevicePreference, candidates};
use super::openvino::{OpenVinoGraph, OpenVinoRuntime};
use super::snake::SnakeOp;

pub struct Runtime {
    env: Environment,
    threads: usize,
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
        let pool = GlobalThreadPoolOptions::default()
            .with_intra_threads(threads)?
            .with_inter_threads(1)?
            .with_spin_control(false)?;
        let env = ort::init()
            .with_name("irodori")
            .with_global_thread_pool(pool)
            .build()?;
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
            env,
            threads,
            preference,
            devices,
            openvino,
        })
    }

    pub fn devices(&self) -> &[DeviceInfo] {
        &self.devices
    }

    pub fn load(&self, path: &Path) -> Result<Graph> {
        self.load_with(path, false)
    }

    pub fn load_spinning(&self, path: &Path) -> Result<Graph> {
        self.load_with(path, true)
    }

    fn load_with(&self, path: &Path, spinning: bool) -> Result<Graph> {
        let cpu = CpuGraph {
            env: self.env.clone(),
            threads: self.threads,
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
    env: Environment,
    threads: usize,
    path: PathBuf,
    spinning: bool,
}

impl CpuGraph {
    fn load(&self) -> Result<Session> {
        let build = || -> ort::Result<Session> {
            let mut builder = Session::builder(&self.env)?;
            if self.spinning {
                builder = builder
                    .with_independent_thread_pool()?
                    .with_intra_threads(self.threads)?
                    .with_intra_op_spinning(true)?;
            }
            builder
                .with_optimization_level(GraphOptimizationLevel::Level3)?
                .with_operators(SnakeOp::domain()?)?
                .commit_from_file(&self.path)
        };
        build().with_context(|| format!("loading CPU graph {}", self.path.display()))
    }
}

pub enum Input<'a> {
    F32(&'a ArrayD<f32>),
    I64(&'a ArrayD<i64>),
    Bool(&'a ArrayD<bool>),
}

enum Backend {
    Ort(Session),
    OpenVino(OpenVinoGraph),
    #[cfg(test)]
    Failed,
}

pub struct Graph {
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
        let session = match backend {
            #[cfg(test)]
            Backend::Failed => bail!("simulated accelerator failure"),
            Backend::OpenVino(graph) => return graph.run(inputs, outputs),
            Backend::Ort(session) => session,
        };
        let mut feeds: Vec<(&str, SessionInputValue<'_>)> = Vec::with_capacity(inputs.len());
        for &(name, ref v) in inputs {
            if !session.inputs().iter().any(|i| i.name() == name) {
                continue;
            }
            let value: SessionInputValue<'_> = match v {
                Input::F32(a) => TensorRef::from_array_view(a.view()).map(Into::into),
                Input::I64(a) => TensorRef::from_array_view(a.view()).map(Into::into),
                Input::Bool(a) => TensorRef::from_array_view(a.view()).map(Into::into),
            }
            .with_context(|| format!("input {name}"))?;
            feeds.push((name, value));
        }
        let out = session.run(feeds)?;
        let mut arrays = Vec::with_capacity(N);
        for name in outputs {
            arrays.push(out[name].try_extract_array::<f32>()?.to_owned());
        }
        Ok(arrays.try_into().expect("one array per output name"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: &[u8] = b"\x08\x07\x12\nonnx-wally:j\n*\n\x06inputs\x12\x07outputs\x1a\ridentity_node\"\x08Identity\x12\x0bno-op-modelZ\x16\n\x06inputs\x12\x0c\n\n\x08\x01\x12\x06\n\x00\n\x02\x08\x02b\x17\n\x07outputs\x12\x0c\n\n\x08\x01\x12\x06\n\x00\n\x02\x08\x02B\x02\x10\x0c";

    #[test]
    fn auto_falls_back_but_explicit_accelerator_fails() {
        let dir = std::env::temp_dir().join(format!("irodori-device-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("identity.onnx");
        std::fs::write(&path, IDENTITY).unwrap();
        let mut runtime = Runtime::discover(1, DevicePreference::Cpu, None).unwrap();
        runtime.devices.push(DeviceInfo {
            kind: DevicePreference::Gpu,
            engine: "test".into(),
            name: "GPU".into(),
            description: String::new(),
            index: 1,
        });
        runtime.preference = DevicePreference::Auto;
        let mut graph = runtime.load(&path).unwrap();
        assert!(matches!(graph.session, Backend::Ort(_)));
        let input =
            ndarray::Array::from_shape_vec(ndarray::IxDyn(&[1, 2]), vec![0.25, -0.5]).unwrap();
        let [actual] = graph
            .run(&[("inputs", Input::F32(&input))], ["outputs"])
            .unwrap();
        assert_eq!(actual, input);
        runtime.preference = DevicePreference::Gpu;
        assert!(runtime.load(&path).is_err());

        let mut graph = Graph {
            session: Backend::Failed,
            fallback: Some(CpuGraph {
                env: runtime.env.clone(),
                threads: 1,
                path: path.clone(),
                spinning: false,
            }),
        };
        let [actual] = graph
            .run(&[("inputs", Input::F32(&input))], ["outputs"])
            .unwrap();
        assert_eq!(actual, input);
        assert!(matches!(graph.session, Backend::Ort(_)));
        assert!(graph.fallback.is_none());
        let mut forced = Graph {
            session: Backend::Failed,
            fallback: None,
        };
        assert!(
            forced
                .run(&[("inputs", Input::F32(&input))], ["outputs"])
                .is_err()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
