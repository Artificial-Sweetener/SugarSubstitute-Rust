// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain fatal native GPU faults and scope synchronous surface-configuration validation.

use crate::GpuError;
use std::{
    cell::Cell,
    error::Error,
    fmt,
    ptr::NonNull,
    sync::{Arc, Mutex},
};

type SharedCause = Arc<dyn Error + Send + Sync>;

#[derive(Default)]
struct State {
    fatal: Option<SharedCause>,
    configuration: Option<ConfigurationCapture>,
}

struct ConfigurationCapture {
    validation: Option<SharedCause>,
}

std::thread_local! {
    /// Identify only the synchronous callback scope on this thread; this creates no worker.
    static CONFIGURING: Cell<Option<NonNull<Faults>>> = const { Cell::new(None) };
}

struct ConfigurationScope<'a> {
    faults: &'a Faults,
    previous: Option<NonNull<Faults>>,
    active: bool,
}

#[derive(Default)]
pub(crate) struct Faults(Mutex<State>);

impl Faults {
    pub(crate) fn record(&self, cause: wgpu::Error) {
        let validation = matches!(&cause, wgpu::Error::Validation { .. });
        let cause: SharedCause = Arc::new(cause);
        let mut state = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if validation
            && let Some(capture) = state.configuration.as_mut()
            && CONFIGURING.get() == Some(NonNull::from(self))
        {
            if capture.validation.is_none() {
                capture.validation = Some(cause);
            } else {
                tracing::error!(operation="image_gpu_secondary_configuration_fault", cause=%cause,
                    "Additional native GPU surface-configuration failure");
            }
            return;
        }
        Self::record_fatal(&mut state, cause);
    }

    pub(crate) fn lost(&self, reason: wgpu::DeviceLostReason, message: String) {
        let mut state = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        Self::record_fatal(&mut state, Arc::new(Lost { reason, message }));
    }

    /// Run native surface configuration while capturing its synchronous validation callback.
    ///
    /// The pinned native wgpu-core backend invokes the uncaptured-error callback before
    /// `Surface::configure` returns. A callback delivered later or on another thread remains a
    /// fatal device fault rather than being mistaken for a recoverable configuration failure.
    pub(crate) fn configure(&self, configure: impl FnOnce()) -> Result<(), GpuError> {
        {
            let mut state = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.configuration.is_some() {
                return Err(GpuError::invalid("Nested GPU surface configuration is forbidden"));
            }
            state.configuration = Some(ConfigurationCapture { validation: None });
        }
        let previous = CONFIGURING.replace(Some(NonNull::from(self)));
        let scope = ConfigurationScope { faults: self, previous, active: true };
        configure();
        let (validation, fatal) = scope.finish();
        match (fatal, validation) {
            (Some(cause), _) => Err(GpuError::new("gpu_native", Shared(cause))),
            (None, Some(cause)) => Err(GpuError::new("gpu_surface_configuration", Shared(cause))),
            (None, None) => Ok(()),
        }
    }

    pub(crate) fn check(&self) -> Result<(), GpuError> {
        let state = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        match state.fatal.as_ref() {
            Some(cause) => Err(GpuError::new("gpu_native", Shared(cause.clone()))),
            None => Ok(()),
        }
    }

    fn record_fatal(state: &mut State, cause: SharedCause) {
        if state.fatal.is_none() {
            state.fatal = Some(cause);
        } else {
            tracing::error!(operation="image_gpu_secondary_fault", cause=%cause,
                "Additional native GPU failure");
        }
    }
}

impl ConfigurationScope<'_> {
    fn finish(mut self) -> (Option<SharedCause>, Option<SharedCause>) {
        let outcome = {
            let mut state = self.faults.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            (state.configuration.take().and_then(|capture| capture.validation), state.fatal.clone())
        };
        CONFIGURING.set(self.previous);
        self.active = false;
        outcome
    }
}

impl Drop for ConfigurationScope<'_> {
    fn drop(&mut self) {
        if self.active {
            let mut state = self.faults.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            state.configuration.take();
            CONFIGURING.set(self.previous);
        }
    }
}

#[derive(Debug)]
struct Shared(SharedCause);
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

    #[derive(Debug)]
    struct Native(&'static str);
    impl fmt::Display for Native {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(self.0)
        }
    }
    impl Error for Native {}

    fn validation(message: &'static str) -> wgpu::Error {
        wgpu::Error::Validation { source: Box::new(Native(message)), description: message.into() }
    }

    #[test]
    fn configuration_validation_is_returned_without_poisoning_the_device() -> Result<(), GpuError> {
        let faults = Faults::default();
        let failure = faults
            .configure(|| faults.record(validation("surface busy")))
            .err()
            .ok_or_else(|| GpuError::invalid("Configuration validation was not returned"))?;
        assert!(failure.to_string().contains("surface busy"));
        faults.check()
    }

    #[test]
    fn uncaptured_validation_remains_a_fatal_device_fault() -> Result<(), GpuError> {
        let faults = Faults::default();
        faults.record(validation("unrelated validation"));
        let failure = faults
            .check()
            .err()
            .ok_or_else(|| GpuError::invalid("Uncaptured validation was not fatal"))?;
        assert!(failure.to_string().contains("unrelated validation"));
        Ok(())
    }

    #[test]
    fn device_loss_during_configuration_remains_fatal() -> Result<(), GpuError> {
        let faults = Faults::default();
        let failure = faults
            .configure(|| faults.lost(wgpu::DeviceLostReason::Unknown, "device removed".into()))
            .err()
            .ok_or_else(|| GpuError::invalid("Device loss was scoped as configuration"))?;
        assert!(failure.to_string().contains("device removed"));
        assert!(faults.check().is_err());
        Ok(())
    }

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
