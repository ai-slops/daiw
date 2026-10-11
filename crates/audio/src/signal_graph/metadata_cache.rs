use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::AudioError;
use crate::source::{SourceMetadata, SourceMetadataProvider};

pub(super) struct CachedMetadataProvider<'a, P> {
    provider: &'a P,
    successful: RefCell<BTreeMap<String, SourceMetadata>>,
}

impl<'a, P> CachedMetadataProvider<'a, P> {
    pub(super) const fn new(provider: &'a P) -> Self {
        Self {
            provider,
            successful: RefCell::new(BTreeMap::new()),
        }
    }
}

impl<P: SourceMetadataProvider> SourceMetadataProvider for CachedMetadataProvider<'_, P> {
    fn metadata(&self, asset_id: &str) -> Result<SourceMetadata, AudioError> {
        if let Some(metadata) = self.successful.borrow().get(asset_id) {
            return Ok(*metadata);
        }
        let metadata = self.provider.metadata(asset_id)?;
        self.successful
            .borrow_mut()
            .insert(asset_id.to_owned(), metadata);
        Ok(metadata)
    }
}
