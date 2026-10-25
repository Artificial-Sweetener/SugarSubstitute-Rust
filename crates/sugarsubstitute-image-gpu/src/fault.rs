// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the first native GPU fault without unwinding through a driver callback.
use crate::GpuError;
use std::{
    error::Error,
    fmt,
    sync::{Arc, Mutex},
};

#[derive(Default)]
pub(crate) struct Faults(Mutex<Option<Arc<dyn Error + Send + Sync>>>);
impl Faults {
    pub(crate) fn record(&self, cause: impl Error + Send + Sync + 'static) {
        let mut slot = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot.is_none() {
            *slot = Some(Arc::new(cause));
        } else {
            tracing::error!(operation="image_gpu_secondary_fault", cause=%cause, "Additional native GPU failure");
        }
    }
    pub(crate) fn lost(&self, reason: wgpu::DeviceLostReason, message: String) {
        self.record(Lost { reason, message });
    }
    pub(crate) fn check(&self) -> Result<(), GpuError> {
        let slot = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        match slot.as_ref() {
            Some(cause) => Err(GpuError::new("gpu_native", Shared(cause.clone()))),
            None => Ok(()),
        }
    }
}
#[derive(Debug)]
struct Shared(Arc<dyn Error + Send + Sync>);
impl fmt::Display for Shared {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug)]
struct Lost {
    reason: wgpu::DeviceLostReason,
    message: String,
}
impl fmt::Display for Lost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "GPU device lost ({:?}): {}", self.reason, self.message)
    }
}
impl Error for Lost {}
impl Error for Shared {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.0.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_loss_retains_the_first_reason_and_original_message() -> Result<(), Box<dyn Error>> {
        let faults = Faults::default();
        faults.check()?;
        faults.lost(wgpu::DeviceLostReason::Unknown, "driver reset".into());
        faults.lost(wgpu::DeviceLostReason::Destroyed, "later shutdown".into());
        let first = faults.check().err().ok_or("Missing device-loss failure")?;
        assert!(first.to_string().contains("driver reset"));
        assert!(!first.to_string().contains("later shutdown"));
        let original = first.source().and_then(Error::source).ok_or("Missing original cause")?;
        assert!(original.downcast_ref::<Lost>().is_some());
        assert_eq!(
            faults.check().err().ok_or("Lost failure disappeared")?.to_string(),
            first.to_string()
        );
        Ok(())
    }
}
