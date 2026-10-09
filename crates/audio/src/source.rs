use std::collections::BTreeMap;

use crate::{AudioError, PcmAsset};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceMetadata {
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: u64,
}

pub trait SourceBank {
    fn metadata(&self, asset_id: &str) -> Result<SourceMetadata, AudioError>;

    fn read_frame(
        &mut self,
        asset_id: &str,
        frame: u64,
        output: &mut [f32; 8],
    ) -> Result<bool, AudioError>;
}

pub struct ResidentSourceBank<'a> {
    assets: &'a BTreeMap<String, PcmAsset>,
}

impl<'a> ResidentSourceBank<'a> {
    #[must_use]
    pub const fn new(assets: &'a BTreeMap<String, PcmAsset>) -> Self {
        Self { assets }
    }
}

impl SourceBank for ResidentSourceBank<'_> {
    fn metadata(&self, asset_id: &str) -> Result<SourceMetadata, AudioError> {
        let asset = self
            .assets
            .get(asset_id)
            .ok_or_else(|| AudioError::MissingAsset {
                asset_id: asset_id.to_owned(),
            })?;
        let frames = u64::try_from(asset.samples.len() / usize::from(asset.channels))?;
        Ok(SourceMetadata {
            sample_rate: asset.sample_rate,
            channels: asset.channels,
            frames,
        })
    }

    fn read_frame(
        &mut self,
        asset_id: &str,
        frame: u64,
        output: &mut [f32; 8],
    ) -> Result<bool, AudioError> {
        let asset = self
            .assets
            .get(asset_id)
            .ok_or_else(|| AudioError::MissingAsset {
                asset_id: asset_id.to_owned(),
            })?;
        let channels = usize::from(asset.channels);
        let start = usize::try_from(frame)?
            .checked_mul(channels)
            .ok_or(AudioError::InvalidFrameCount)?;
        let Some(samples) = asset.samples.get(start..start + channels) else {
            return Ok(false);
        };
        output[..channels].copy_from_slice(samples);
        Ok(true)
    }
}
