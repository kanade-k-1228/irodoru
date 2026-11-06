use ort::operator::{
    BoxedKernel, ComputeContext, KernelContext, Operator, OperatorDomain, OperatorInput,
    OperatorOutput,
};
use ort::value::TensorElementType;

const DOMAIN: &str = "ai.onnx.contrib";

pub struct SnakeOp;

impl SnakeOp {
    pub fn domain() -> ort::Result<OperatorDomain> {
        OperatorDomain::new(DOMAIN)?.add(SnakeOp)
    }
}

struct RowPtr(*mut f32);
unsafe impl Send for RowPtr {}
unsafe impl Sync for RowPtr {}

impl RowPtr {
    fn get(&self) -> *mut f32 {
        self.0
    }
}

#[inline]
fn snake_row(x: &[f32], y: &mut [f32], a: f32) {
    let inv = 1.0 / (a + 1e-9);
    for (yo, &v) in y.iter_mut().zip(x) {
        let s = (a * v).sin();
        *yo = v + inv * s * s;
    }
}

impl Operator for SnakeOp {
    type Kernel<'attr> = BoxedKernel<'attr>;

    fn name(&self) -> &str {
        "Snake"
    }

    fn inputs(&self) -> impl IntoIterator<Item = OperatorInput> {
        [
            OperatorInput::required(TensorElementType::Float32),
            OperatorInput::required(TensorElementType::Float32),
        ]
    }

    fn outputs(&self) -> impl IntoIterator<Item = OperatorOutput> {
        [OperatorOutput::required(TensorElementType::Float32)]
    }

    fn create_kernel<'attr>(&self, _: &KernelContext<'attr>) -> ort::Result<Self::Kernel<'attr>> {
        Ok(Box::new(|ctx: &ComputeContext| {
            let missing = || ort::Error::new("Snake: missing input");
            let x_val = ctx.input(0)?.ok_or_else(missing)?;
            let a_val = ctx.input(1)?.ok_or_else(missing)?;
            let (shape, x) = x_val.try_extract_tensor::<f32>()?;
            let (_, alpha) = a_val.try_extract_tensor::<f32>()?;
            let dims: Vec<i64> = shape.to_vec();
            if dims.len() < 2 {
                return Err(ort::Error::new("Snake: input must be (B, C, ...)"));
            }
            let channels = dims[1] as usize;
            if alpha.len() != channels {
                return Err(ort::Error::new(
                    "Snake: alpha must have one value per channel",
                ));
            }
            let rows = (dims[0] as usize) * channels;
            let row_len = x.len().checked_div(rows).unwrap_or(0);

            let mut out = ctx
                .output(0, dims)?
                .ok_or_else(|| ort::Error::new("Snake: missing output"))?;
            let (_, y) = out.try_extract_tensor_mut::<f32>()?;
            let y_ptr = RowPtr(y.as_mut_ptr());
            ctx.par_for(rows, rows, |r| {
                let start = r * row_len;
                // SAFETY: rows are disjoint and `y` outlives this synchronous parallel loop.
                let y_row =
                    unsafe { std::slice::from_raw_parts_mut(y_ptr.get().add(start), row_len) };
                snake_row(&x[start..start + row_len], y_row, alpha[r % channels]);
            })?;
            Ok(())
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::snake_row;

    #[test]
    fn matches_reference_formula() {
        let x = [-2.0f32, -0.5, 0.0, 0.3, 1.7];
        let a = 0.8f32;
        let mut y = [0.0f32; 5];
        snake_row(&x, &mut y, a);
        for (v, got) in x.iter().zip(y) {
            let want = (*v as f64) + ((a as f64) * (*v as f64)).sin().powi(2) / (a as f64 + 1e-9);
            assert!((got as f64 - want).abs() < 1e-6, "{got} vs {want}");
        }
    }
}
