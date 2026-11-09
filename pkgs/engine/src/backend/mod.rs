pub(crate) mod device;
mod onnx;
mod runtime;
pub(crate) use runtime::Graph;
pub use runtime::Runtime;

pub(crate) enum Input<'a> {
    F32(&'a ndarray::ArrayD<f32>),
    I64(&'a ndarray::ArrayD<i64>),
    Bool(&'a ndarray::ArrayD<bool>),
}

#[cfg(feature = "openvino")]
mod openvino;
mod snake;
#[cfg(not(feature = "openvino"))]
mod openvino {
    use super::{Input, device::DeviceInfo};
    use anyhow::{Result, bail};
    use std::path::Path;
    pub struct OpenVinoRuntime;
    pub struct OpenVinoGraph;
    impl OpenVinoRuntime {
        pub fn discover(_: Option<&Path>) -> Result<(Self, Vec<DeviceInfo>)> {
            bail!("OpenVINO support is not enabled in this build")
        }
        pub fn load(&self, _: &Path, _: &str) -> Result<OpenVinoGraph> {
            bail!("OpenVINO support is not enabled in this build")
        }
    }
    impl OpenVinoGraph {
        pub fn run<const N: usize>(
            &mut self,
            _: &[(&str, Input<'_>)],
            _: [&str; N],
        ) -> Result<[ndarray::ArrayD<f32>; N]> {
            bail!("OpenVINO support is not enabled in this build")
        }
    }
}
