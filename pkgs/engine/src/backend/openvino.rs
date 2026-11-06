use std::{
    cell::RefCell,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use ndarray::{ArrayD, IxDyn};
use openvino::{
    CompiledModel, Core, DeviceType, ElementType, InferRequest, PropertyKey, RwPropertyKey, Shape,
    Tensor,
};

use super::{
    device::{DeviceInfo, DevicePreference},
    onnx::Input,
};

pub(super) struct OpenVinoRuntime {
    core: RefCell<Core>,
}

impl OpenVinoRuntime {
    pub(super) fn discover(library_path: Option<&Path>) -> Result<(Self, Vec<DeviceInfo>)> {
        let library = library_path.map(Path::to_path_buf).or_else(find_openvino);
        if let Some(path) = library {
            openvino_sys::library::load_from(path).map_err(anyhow::Error::msg)?;
        }
        let core = Core::new()?;
        let devices = Self::discover_devices(&core)?;
        Ok((
            Self {
                core: RefCell::new(core),
            },
            devices,
        ))
    }

    fn discover_devices(core: &Core) -> Result<Vec<DeviceInfo>> {
        let mut devices = Vec::new();
        for device in core.available_devices()? {
            let name = device.to_string();
            let kind = if name.starts_with("GPU") {
                DevicePreference::Gpu
            } else if name.starts_with("NPU") {
                DevicePreference::Npu
            } else {
                continue;
            };
            devices.push(DeviceInfo {
                index: devices.len() + 1,
                kind,
                engine: "openvino".into(),
                description: core
                    .get_property(&device, &PropertyKey::DeviceFullName)
                    .unwrap_or_default(),
                name,
            });
        }
        Ok(devices)
    }

    pub(super) fn load(&self, path: &Path, device: &str) -> Result<OpenVinoGraph> {
        OpenVinoGraph::load(&mut self.core.borrow_mut(), path, device)
    }
}

fn find_openvino() -> Option<PathBuf> {
    let name = if cfg!(target_os = "windows") {
        "openvino_c.dll"
    } else {
        "libopenvino_c.so"
    };
    let cwd = std::env::current_dir().ok()?;
    let mut roots = vec![cwd.join(".runtime/openvino")];
    if let Some(parent) = cwd.parent() {
        roots.push(parent.join(".runtime/openvino"));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        roots.extend([parent.to_owned(), parent.join("runtime/openvino")]);
    }
    roots
        .into_iter()
        .map(|root| root.join(name))
        .find(|p| p.is_file())
}

pub struct OpenVinoGraph {
    request: InferRequest,
    _model: CompiledModel,
    inputs: Vec<String>,
}

impl OpenVinoGraph {
    pub fn load(core: &mut Core, path: &Path, device: &str) -> Result<Self> {
        let device_type = DeviceType::from(device);
        core.set_property(&device_type, &RwPropertyKey::HintPerformanceMode, "LATENCY")?;
        if device.starts_with("GPU") {
            core.set_property(&device_type, &RwPropertyKey::HintInferencePrecision, "f32")?;
        }
        let source = core
            .read_model_from_file(path.to_str().context("non-UTF8 model path")?, "")
            .with_context(|| format!("reading OpenVINO graph {}", path.display()))?;
        let mut model = core
            .compile_model(&source, device_type)
            .with_context(|| format!("compiling {} for {device}", path.display()))?;
        let executing = model.get_property(&PropertyKey::Other("EXECUTION_DEVICES".into()))?;
        ensure!(
            executing.contains(device),
            "OpenVINO selected {executing} instead of requested {device}"
        );
        let inputs = (0..model.get_input_size()?)
            .map(|i| model.get_input_by_index(i)?.get_name())
            .collect::<Result<Vec<_>, openvino::InferenceError>>()?;
        let request = model.create_infer_request()?;
        Ok(Self {
            request,
            _model: model,
            inputs,
        })
    }

    pub fn run<const N: usize>(
        &mut self,
        inputs: &[(&str, Input<'_>)],
        outputs: [&str; N],
    ) -> Result<[ArrayD<f32>; N]> {
        let mut tensors = Vec::new();
        for &(name, ref input) in inputs {
            if self._model.get_input_by_name(name).is_err() {
                continue;
            }
            let (dims, element) = match input {
                Input::F32(a) => (a.shape(), ElementType::F32),
                Input::I64(a) => (a.shape(), ElementType::I64),
                Input::Bool(a) => (a.shape(), ElementType::Boolean),
            };
            let dims = dims
                .iter()
                .map(|&n| i64::try_from(n))
                .collect::<Result<Vec<_>, _>>()?;
            let mut tensor = Tensor::new(element, &Shape::new(&dims)?)?;
            match input {
                Input::F32(a) => tensor
                    .get_data_mut::<f32>()?
                    .iter_mut()
                    .zip(a.iter())
                    .for_each(|(y, &x)| *y = x),
                Input::I64(a) => tensor
                    .get_data_mut::<i64>()?
                    .iter_mut()
                    .zip(a.iter())
                    .for_each(|(y, &x)| *y = x),
                Input::Bool(a) => tensor
                    .get_data_mut::<u8>()?
                    .iter_mut()
                    .zip(a.iter())
                    .for_each(|(y, &x)| *y = u8::from(x)),
            }
            self.request
                .set_tensor(name, &tensor)
                .with_context(|| format!("OpenVINO input {name}"))?;
            tensors.push(tensor);
        }
        ensure!(
            tensors.len() == self.inputs.len(),
            "OpenVINO inputs were not all supplied: expected {:?}, supplied {:?}",
            self.inputs,
            inputs.iter().map(|(name, _)| *name).collect::<Vec<_>>()
        );
        self.request
            .infer()
            .with_context(|| format!("OpenVINO inference, inputs {:?}", self.inputs))?;
        let mut arrays = Vec::with_capacity(N);
        for name in outputs {
            let tensor = self
                .request
                .get_tensor(name)
                .with_context(|| format!("OpenVINO output {name}"))?;
            ensure!(
                tensor.get_element_type()? == ElementType::F32,
                "non-F32 output {name}"
            );
            let shape = tensor.get_shape()?;
            let dims = shape
                .get_dimensions()
                .iter()
                .map(|&n| usize::try_from(n))
                .collect::<Result<Vec<_>, _>>()?;
            arrays.push(ArrayD::from_shape_vec(
                IxDyn(&dims),
                tensor.get_data::<f32>()?.to_vec(),
            )?);
        }
        Ok(arrays.try_into().expect("one array per output name"))
    }
}
