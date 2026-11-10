use std::{fmt, str::FromStr};

use anyhow::{Result, bail};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DevicePreference {
    #[default]
    Auto,
    Cpu,
    Gpu,
    Npu,
}

impl FromStr for DevicePreference {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "auto" => Ok(Self::Auto),
            "cpu" => Ok(Self::Cpu),
            "gpu" => Ok(Self::Gpu),
            "npu" => Ok(Self::Npu),
            _ => bail!("unknown device {value:?}; expected auto, cpu, gpu or npu"),
        }
    }
}

impl fmt::Display for DevicePreference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Auto => "auto",
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::Npu => "npu",
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct DeviceInfo {
    pub kind: DevicePreference,
    pub engine: String,
    pub name: String,
    pub description: String,
    #[serde(skip)]
    pub index: usize,
}

pub fn candidates(preference: DevicePreference, devices: &[DeviceInfo]) -> Vec<&DeviceInfo> {
    let mut selected: Vec<_> = devices
        .iter()
        .filter(|d| match preference {
            DevicePreference::Auto => {
                matches!(d.kind, DevicePreference::Gpu | DevicePreference::Npu)
            }
            DevicePreference::Cpu => false,
            other => d.kind == other,
        })
        .collect();
    selected.sort_by_key(|d| (d.kind == DevicePreference::Npu, d.index));
    selected
}
