use std::collections::BTreeMap;

use crate::source::{SourceMetadata, SourceMetadataProvider};
use crate::{AudioError, PcmAsset};

pub struct ResidentDspSources<'a> {
    pub(crate) assets: &'a BTreeMap<String, PcmAsset>,
}

impl<'a> ResidentDspSources<'a> {
    #[must_use]
    pub const fn new(assets: &'a BTreeMap<String, PcmAsset>) -> Self {
        Self { assets }
    }
}

impl SourceMetadataProvider for ResidentDspSources<'_> {
    fn metadata(&self, asset_id: &str) -> Result<SourceMetadata, AudioError> {
        let asset = self
            .assets
            .get(asset_id)
            .ok_or_else(|| AudioError::MissingAsset {
                asset_id: asset_id.to_owned(),
            })?;
        if asset.channels == 0 {
            return Err(AudioError::InvalidAssetChannels {
                asset_id: asset_id.to_owned(),
                channels: asset.channels,
            });
        }
        Ok(SourceMetadata {
            sample_rate: asset.sample_rate,
            channels: asset.channels,
            frames: u64::try_from(asset.samples.len() / usize::from(asset.channels))?,
        })
    }
}
