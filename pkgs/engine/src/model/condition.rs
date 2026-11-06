use anyhow::Result;
use ndarray::{ArrayD, Axis, IxDyn, concatenate};

#[derive(Clone)]
pub struct Condition {
    pub(super) state: ArrayD<f32>,
    pub(super) mask: ArrayD<bool>,
}

impl Condition {
    pub(crate) fn byte_size(&self) -> usize {
        self.state.len() * size_of::<f32>() + self.mask.len()
    }
    pub(super) fn empty(dim: usize) -> Self {
        Self {
            state: ArrayD::zeros(IxDyn(&[1, 1, dim])),
            mask: ArrayD::from_elem(IxDyn(&[1, 1]), false),
        }
    }

    pub(super) fn is_present(&self) -> bool {
        self.mask.iter().any(|&m| m)
    }

    pub(super) fn dropped(&self) -> Self {
        Self {
            state: ArrayD::zeros(self.state.raw_dim()),
            mask: ArrayD::from_elem(self.mask.raw_dim(), false),
        }
    }

    pub(super) fn concat(conds: &[&Condition]) -> Result<Self> {
        let states: Vec<_> = conds.iter().map(|c| c.state.view()).collect();
        let masks: Vec<_> = conds.iter().map(|c| c.mask.view()).collect();
        Ok(Self {
            state: concatenate(Axis(0), &states)?,
            mask: concatenate(Axis(0), &masks)?,
        })
    }
}

pub(super) fn valid_mask(len: usize) -> ArrayD<bool> {
    ArrayD::from_elem(IxDyn(&[1, len]), true)
}

#[cfg(test)]
impl Condition {
    pub(crate) fn test_value(value: f32) -> Self {
        Self {
            state: ArrayD::from_elem(IxDyn(&[1, 1, 1]), value),
            mask: valid_mask(1),
        }
    }
    pub(crate) fn test_scalar(&self) -> f32 {
        self.state[[0, 0, 0]]
    }
    pub(crate) fn test_fill(&mut self, value: f32) {
        self.state.fill(value);
    }
}
