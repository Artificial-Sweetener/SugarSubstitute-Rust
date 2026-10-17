// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Attribute retained custody errors without repeated telemetry or replacing their causes.
use std::{error::Error, fmt};
use sugarsubstitute_execution::{
    windows::NativeObservationError, windows_lifecycle_pipe::LifecyclePipeFailure,
};
use sugarsubstitute_supervision::BackendCustodyParentFailure;

/// Fixed invoking transition plus the retained typed native failure's closed metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production) struct CustodyFailureObservation {
    pub(in crate::production) transition: &'static str,
    pub(in crate::production) native_operation: Option<&'static str>,
    pub(in crate::production) native_code: Option<u32>,
}
/// Read only reviewed concrete owners; unknown Error callbacks and payloads stay untouched.
pub(in crate::production) fn observation(
    cause: &(dyn Error + 'static),
) -> Option<CustodyFailureObservation> {
    let cause = cause.downcast_ref::<CleanupFailed>().map_or(cause, |owner| owner.primary.as_ref());
    let (transition, cause) = if let Some(owner) = cause.downcast_ref::<TransitionFailed>() {
        (owner.transition, owner.cause.as_ref() as &(dyn Error + 'static))
    } else if cause.is::<BackendCustodyParentFailure>() {
        ("custody_parent", cause)
    } else if cause.is::<LifecyclePipeFailure>() {
        ("custody_pipe_terminal", cause)
    } else {
        return None;
    };
    let native = if let Some(parent) = cause.downcast_ref::<BackendCustodyParentFailure>() {
        parent.source().and_then(|cause| cause.downcast_ref::<NativeObservationError>())
    } else if let Some(pipe) = cause.downcast_ref::<LifecyclePipeFailure>() {
        Some(pipe.native_cause())
    } else {
        cause.downcast_ref::<NativeObservationError>()
    };
    Some(CustodyFailureObservation {
        transition,
        native_operation: native.map(NativeObservationError::stage),
        native_code: native.and_then(NativeObservationError::win32_code),
    })
}
/// The original failure and its actual invoking transition remain inseparable.
#[derive(Debug)]
struct TransitionFailed {
    transition: &'static str,
    cause: Box<dyn Error + Send + Sync>,
}
/// Attach a fixed developer transition label to the original owned boundary failure.
pub(super) fn failed(
    transition: &'static str,
    cause: impl Error + Send + Sync + 'static,
) -> Box<dyn Error + Send + Sync> {
    Box::new(TransitionFailed { transition, cause: Box::new(cause) })
}
impl fmt::Display for TransitionFailed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Custody transition {} failed: {}", self.transition, self.cause)
    }
}
impl Error for TransitionFailed {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

/// Preserve the first protocol failure while retaining the actual cleanup failure too.
pub(super) fn preserve_cleanup(
    primary: Option<Box<dyn Error + Send + Sync>>,
    cleanup: Box<dyn Error + Send + Sync>,
) -> Box<dyn Error + Send + Sync> {
    match primary {
        Some(primary) => Box::new(CleanupFailed { primary, cleanup }),
        None => cleanup,
    }
}
/// Both owned failures remain bounded; source identifies the original operation.
#[derive(Debug)]
struct CleanupFailed {
    primary: Box<dyn Error + Send + Sync>,
    cleanup: Box<dyn Error + Send + Sync>,
}
impl fmt::Display for CleanupFailed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}; cleanup failed: {}", self.primary, self.cleanup)
    }
}
impl Error for CleanupFailed {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.primary.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    #[test]
    fn custody_transition_diagnostic_preserves_actual_native_cause_and_status()
    -> Result<(), Box<dyn Error>> {
        let native =
            NativeObservationError::new("pipe_completion", std::io::Error::from_raw_os_error(109));
        let original = native.source().map(std::ptr::from_ref);
        let cause = failed("profile_retirement_observation", native);
        let projected = observation(cause.as_ref()).ok_or("Typed custody observation absent")?;
        assert_eq!(projected.transition, "profile_retirement_observation");
        assert_eq!(projected.native_operation, Some("pipe_completion"));
        assert_eq!(projected.native_code, Some(109));
        let retained = cause
            .source()
            .and_then(|source| source.downcast_ref::<NativeObservationError>())
            .ok_or("Actual native cause lost")?;
        assert_eq!(retained.source().map(std::ptr::from_ref), original);
        let combined =
            preserve_cleanup(Some(cause), Box::new(std::io::Error::from_raw_os_error(5)));
        assert_eq!(observation(combined.as_ref()), Some(projected));
        assert_eq!(
            combined
                .source()
                .and_then(Error::source)
                .and_then(|source| source.downcast_ref::<NativeObservationError>())
                .and_then(Error::source)
                .map(std::ptr::from_ref),
            original
        );
        Ok(())
    }
    struct Unknown {
        callbacks: Arc<AtomicUsize>,
        dropped: Arc<AtomicUsize>,
    }
    impl fmt::Debug for Unknown {
        fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.callbacks.fetch_add(1, Ordering::SeqCst);
            Err(fmt::Error)
        }
    }
    impl fmt::Display for Unknown {
        fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.callbacks.fetch_add(1, Ordering::SeqCst);
            Err(fmt::Error)
        }
    }
    impl Error for Unknown {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            self.callbacks.fetch_add(1, Ordering::SeqCst);
            None
        }
    }
    impl Drop for Unknown {
        fn drop(&mut self) {
            self.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[test]
    fn custody_transition_diagnostic_does_not_format_walk_or_release_unknown_leaf() {
        let dropped = Arc::new(AtomicUsize::new(0));
        let callbacks = Arc::new(AtomicUsize::new(0));
        let cause = failed(
            "ready_current_admission",
            Unknown { callbacks: callbacks.clone(), dropped: dropped.clone() },
        );
        let projected = observation(cause.as_ref());
        assert_eq!(
            projected,
            Some(CustodyFailureObservation {
                transition: "ready_current_admission",
                native_operation: None,
                native_code: None
            })
        );
        assert_eq!(callbacks.load(Ordering::SeqCst), 0);
        assert_eq!(dropped.load(Ordering::SeqCst), 0);
        drop(cause);
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
        assert_eq!(callbacks.load(Ordering::SeqCst), 0);
    }
}
