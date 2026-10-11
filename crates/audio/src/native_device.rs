use cpal::traits::{DeviceTrait, HostTrait};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::native::{NativeRenderFormat, select_output_config};
use crate::{AudioError, Sha256Digest, WavStreamError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeDeviceSelector {
    index: usize,
    name_sha256: String,
}

impl NativeDeviceSelector {
    pub fn new(index: usize, name_sha256: &str) -> Result<Self, WavStreamError> {
        Sha256Digest::parse(name_sha256)?;
        Ok(Self {
            index,
            name_sha256: name_sha256.to_owned(),
        })
    }

    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    #[must_use]
    pub fn name_sha256(&self) -> &str {
        &self.name_sha256
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeOutputDevice {
    pub index: usize,
    pub name: String,
    pub name_sha256: String,
    pub is_default: bool,
    pub supported: bool,
}

struct EnumeratedDevice {
    device: cpal::Device,
    descriptor: NativeOutputDevice,
}

pub fn list_native_output_devices() -> Result<Vec<NativeOutputDevice>, AudioError> {
    enumerate(NativeRenderFormat {
        sample_rate: 48_000,
        channels: 2,
    })
    .map(|devices| devices.into_iter().map(|entry| entry.descriptor).collect())
}

pub(crate) fn selected_output_device_and_config(
    selector: &NativeDeviceSelector,
    format: NativeRenderFormat,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig), AudioError> {
    let mut devices = enumerate(format)?;
    let descriptors: Vec<NativeOutputDevice> = devices
        .iter()
        .map(|entry| entry.descriptor.clone())
        .collect();
    if !selector_matches_descriptors(selector, &descriptors) {
        return Err(AudioError::DeviceUnavailable);
    }
    let entry = devices
        .get_mut(selector.index)
        .ok_or(AudioError::DeviceUnavailable)?;
    let supported = entry
        .device
        .supported_output_configs()
        .map_err(|error| AudioError::Native(error.to_string()))?;
    let config = select_output_config(supported, format).ok_or(AudioError::DeviceUnavailable)?;
    Ok((entry.device.clone(), config))
}

fn enumerate(format: NativeRenderFormat) -> Result<Vec<EnumeratedDevice>, AudioError> {
    let host = cpal::default_host();
    let default_name = host
        .default_output_device()
        .and_then(|device| device.name().ok());
    let mut named = host
        .output_devices()
        .map_err(|error| AudioError::Native(error.to_string()))?
        .map(|device| {
            let name = device
                .name()
                .map_err(|error| AudioError::Native(error.to_string()))?;
            Ok((name, device))
        })
        .collect::<Result<Vec<_>, AudioError>>()?;
    named.sort_by(|left, right| left.0.cmp(&right.0));
    let names: Vec<&str> = named.iter().map(|(name, _)| name.as_str()).collect();
    let defaults = default_name_flags(&names, default_name.as_deref());
    let unique = device_name_unique_flags(&names);
    named
        .into_iter()
        .zip(defaults.into_iter().zip(unique))
        .enumerate()
        .map(|(index, ((name, device), (is_default, is_unique)))| {
            let supported = device
                .supported_output_configs()
                .map_err(|error| AudioError::Native(error.to_string()))?;
            Ok(EnumeratedDevice {
                descriptor: NativeOutputDevice {
                    index,
                    name_sha256: output_device_name_sha256(&name),
                    name,
                    is_default,
                    supported: is_unique && select_output_config(supported, format).is_some(),
                },
                device,
            })
        })
        .collect()
}

pub(crate) fn device_name_unique_flags(names: &[&str]) -> Vec<bool> {
    names
        .iter()
        .map(|name| {
            names
                .iter()
                .filter(|candidate| **candidate == *name)
                .count()
                == 1
        })
        .collect()
}

pub(crate) fn selector_matches_descriptors(
    selector: &NativeDeviceSelector,
    descriptors: &[NativeOutputDevice],
) -> bool {
    let Some(selected) = descriptors.get(selector.index) else {
        return false;
    };
    selected.index == selector.index
        && selected.name_sha256 == selector.name_sha256
        && selected.supported
        && descriptors
            .iter()
            .filter(|candidate| candidate.name == selected.name)
            .count()
            == 1
}

pub(crate) fn output_device_name_sha256(name: &str) -> String {
    format!("{:x}", Sha256::digest(name.as_bytes()))
}

pub(crate) fn default_name_flags(names: &[&str], default_name: Option<&str>) -> Vec<bool> {
    names
        .iter()
        .map(|name| {
            default_name.is_some_and(|default| {
                *name == default
                    && names
                        .iter()
                        .filter(|candidate| **candidate == default)
                        .count()
                        == 1
            })
        })
        .collect()
}
