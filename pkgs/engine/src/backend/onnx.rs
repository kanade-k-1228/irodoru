//! ONNX Runtime implementation. Device selection and fallback live in runtime.
use super::{Input, snake::SnakeOp};
use anyhow::{Context, Result};
use ndarray::ArrayD;
use ort::environment::{Environment, GlobalThreadPoolOptions};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::{Session, SessionInputValue};
use ort::value::TensorRef;
use std::{path::Path, sync::Mutex};

// ort allows only one environment creation per process, even after its last
// session is dropped. Keep that environment alive; model sessions remain local
// to each Engine and are still dropped on a model switch.
static ENVIRONMENT: Mutex<Option<(Environment, usize)>> = Mutex::new(None);
// Custom-op schema registration also mutates ONNX Runtime's process-wide state.
// Serialize session construction; running already-created sessions stays independent.
static SESSION_BUILD: Mutex<()> = Mutex::new(());

fn environment(threads: usize) -> Result<(Environment, usize)> {
    let mut shared = ENVIRONMENT
        .lock()
        .map_err(|_| anyhow::anyhow!("ONNX environment lock poisoned"))?;
    if let Some(env) = shared.as_ref() {
        return Ok(env.clone());
    }
    #[cfg(target_os = "windows")]
    let builder = crate::windows_runtime::builder()?;
    #[cfg(not(target_os = "windows"))]
    let builder = ort::init();
    let pool = GlobalThreadPoolOptions::default()
        .with_intra_threads(threads)?
        .with_inter_threads(1)?
        .with_spin_control(false)?;
    let env = builder
        .with_name("irodori")
        .with_global_thread_pool(pool)
        .build()?;
    *shared = Some((env.clone(), threads));
    Ok((env, threads))
}

#[derive(Clone)]
pub(super) struct OrtRuntime {
    env: Environment,
    pool_threads: usize,
    threads: usize,
}
impl OrtRuntime {
    pub fn new(threads: usize) -> Result<Self> {
        let (env, pool_threads) = environment(threads)?;
        Ok(Self {
            env,
            pool_threads,
            threads,
        })
    }
    pub fn load(&self, path: &Path, spinning: bool) -> Result<OrtGraph> {
        self.load_session(path, spinning, None, None)
    }

    #[cfg(target_os = "windows")]
    pub fn devices(&self) -> Result<Vec<super::device::DeviceInfo>> {
        use super::device::{DeviceInfo, DevicePreference};
        use ort::memory::DeviceType;
        crate::windows_runtime::register_providers(&self.env)?;
        self.env
            .devices()
            .enumerate()
            .filter_map(|(index, d)| {
                let kind = match d.hardware_device().ty() {
                    DeviceType::GPU => DevicePreference::Gpu,
                    DeviceType::NPU => DevicePreference::Npu,
                    _ => return None,
                };
                Some((|| -> Result<DeviceInfo> {
                    Ok(DeviceInfo {
                        kind,
                        index,
                        engine: "windows-ml".into(),
                        name: d.ep()?.to_owned(),
                        description: format!(
                            "Windows ML {} ({})",
                            d.ep()?,
                            d.hardware_device().vendor()?
                        ),
                    })
                })())
            })
            .collect()
    }

    #[cfg(target_os = "windows")]
    pub fn load_device(&self, path: &Path, spinning: bool, index: usize) -> Result<OrtGraph> {
        self.load_session(path, spinning, Some(index), None)
    }

    #[cfg(target_os = "macos")]
    pub fn load_coreml(
        &self,
        path: &Path,
        spinning: bool,
        preference: super::device::DevicePreference,
    ) -> Result<OrtGraph> {
        self.load_session(path, spinning, None, Some(preference))
    }

    fn load_session(
        &self,
        path: &Path,
        spinning: bool,
        _device: Option<usize>,
        _coreml: Option<super::device::DevicePreference>,
    ) -> Result<OrtGraph> {
        let _build = SESSION_BUILD
            .lock()
            .map_err(|_| anyhow::anyhow!("ONNX session lock poisoned"))?;
        let build = || -> ort::Result<Session> {
            let mut builder = Session::builder(&self.env)?;
            #[cfg(target_os = "windows")]
            {
                // CPU fallback must not automatically pick a registered accelerator.
                builder =
                    builder.with_auto_device(ort::session::builder::AutoDevicePolicy::PreferCPU)?;
                if let Some(index) = _device {
                    let device = self.env.devices().nth(index).ok_or_else(|| {
                        ort::Error::new("Windows ML device is no longer available")
                    })?;
                    builder = builder
                        .with_devices([device], None)?
                        .with_parallel_execution(false)?
                        .with_memory_pattern(false)?;
                }
            }
            #[cfg(target_os = "macos")]
            if let Some(preference) = _coreml {
                use super::device::DevicePreference;
                use ort::ep::{
                    CoreML,
                    coreml::{ComputeUnits, ModelFormat},
                };
                let units = match preference {
                    DevicePreference::Gpu => ComputeUnits::CPUAndGPU,
                    DevicePreference::Npu => ComputeUnits::CPUAndNeuralEngine,
                    _ => ComputeUnits::All,
                };
                builder = builder.with_execution_providers([CoreML::default()
                    .with_model_format(ModelFormat::MLProgram)
                    .with_compute_units(units)
                    .build()
                    .error_on_failure()])?;
            }
            if spinning || self.threads != self.pool_threads {
                builder = builder
                    .with_independent_thread_pool()?
                    .with_intra_threads(self.threads)?
                    .with_inter_threads(1)?
                    .with_intra_op_spinning(spinning)?;
            }
            builder
                .with_optimization_level(GraphOptimizationLevel::Level3)?
                .with_operators(SnakeOp::domain()?)?
                .commit_from_file(path)
        };
        build()
            .map(OrtGraph)
            .with_context(|| format!("loading ONNX graph {}", path.display()))
    }
}

pub(super) struct OrtGraph(Session);
impl OrtGraph {
    pub fn run<const N: usize>(
        &mut self,
        inputs: &[(&str, Input<'_>)],
        outputs: [&str; N],
    ) -> Result<[ArrayD<f32>; N]> {
        let mut feeds: Vec<(&str, SessionInputValue<'_>)> = Vec::with_capacity(inputs.len());
        for &(name, ref v) in inputs {
            if !self.0.inputs().iter().any(|i| i.name() == name) {
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
        let out = self.0.run(feeds)?;
        let mut arrays = Vec::with_capacity(N);
        for name in outputs {
            arrays.push(out[name].try_extract_array::<f32>()?.to_owned());
        }
        Ok(arrays.try_into().expect("one array per output name"))
    }
}
