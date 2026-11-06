pub mod device;
pub(crate) mod onnx;
#[cfg(feature = "openvino")]
mod openvino;
mod snake;
#[cfg(not(feature = "openvino"))]
mod openvino {
    use super::{device::DeviceInfo, onnx::Input};
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
