use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::control::WavStreamControl;
use super::shared::{Shared, lock};
use super::{WavStreamError, WavStreamStats};

#[derive(Clone)]
pub struct DeviceErrorReporter {
    shared: Arc<Shared>,
}

impl DeviceErrorReporter {
    pub fn record(&self) {
        self.shared.device_errors.fetch_add(1, Ordering::Relaxed);
        let (generation, _) = self.shared.state_snapshot();
        self.shared.fault(generation, WavStreamError::DeviceError);
    }
}

impl WavStreamControl {
    #[must_use]
    pub fn stats(&self) -> WavStreamStats {
        WavStreamStats {
            underrun_callbacks: self.shared.underruns.load(Ordering::Relaxed),
            stale_blocks: self.shared.stale_blocks.load(Ordering::Relaxed),
            device_errors: self.shared.device_errors.load(Ordering::Relaxed),
        }
    }

    #[must_use]
    pub fn device_error_reporter(&self) -> DeviceErrorReporter {
        DeviceErrorReporter {
            shared: Arc::clone(&self.shared),
        }
    }

    pub fn report_device_error(&self) {
        self.device_error_reporter().record();
    }

    pub fn take_error(&self) -> Option<WavStreamError> {
        lock(&self.shared.error).take()
    }
}
