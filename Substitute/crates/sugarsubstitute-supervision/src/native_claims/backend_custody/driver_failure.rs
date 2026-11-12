// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve the first callback error while returning the same native outcome to its worker.
use crate::ClaimError;
use sugarsubstitute_execution::windows::NativeObservationError;

/// Record only a previously unlatched native callback failure in its admitted source slot.
/// Existing first/secondary causes are untouched, including errors recorded inside the callback.
pub(super) fn retain<T>(
    result: Result<T, NativeObservationError>,
    first: &mut Option<ClaimError>,
    failed: &mut bool,
) -> Result<T, NativeObservationError> {
    if let Err(cause) = &result {
        *failed = true;
        if first.is_none() {
            *first = Some(ClaimError::new(cause.stage(), cause.clone()));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn exchange_failure_is_latched_before_worker_return_and_survives_later_cleanup()
    -> Result<(), Box<dyn Error>> {
        for stage in ["custody_receive", "custody_reply", "custody_initialize_clock"] {
            let original =
                NativeObservationError::new(stage, std::io::Error::from_raw_os_error(109));
            let pointer = original.source().map(std::ptr::from_ref);
            let mut first = None;
            let mut failed = false;
            let returned = retain::<()>(Err(original), &mut first, &mut failed)
                .err()
                .ok_or("Native callback error was lost")?;
            assert!(failed);
            assert_eq!(returned.stage(), stage);
            assert_eq!(returned.source().map(std::ptr::from_ref), pointer);
            let retained = first.as_ref().ok_or("Source first cause was not latched")?;
            assert_eq!(retained.operation(), stage);
            assert_eq!(retained.win32_code(), Some(109));
            let nested = retained
                .source()
                .and_then(|cause| cause.downcast_ref::<NativeObservationError>())
                .ok_or("Original native cause was not retained")?;
            assert_eq!(nested.source().map(std::ptr::from_ref), pointer);
            let first_control = retained.clone();
            let cleanup = NativeObservationError::new(
                "custody_retirement",
                std::io::Error::from_raw_os_error(5),
            );
            let cleanup_pointer = cleanup.source().map(std::ptr::from_ref);
            let cleanup = retain::<()>(Err(cleanup), &mut first, &mut failed)
                .err()
                .ok_or("Cleanup callback error was lost")?;
            assert_eq!(cleanup.source().map(std::ptr::from_ref), cleanup_pointer);
            assert!(first.as_ref().is_some_and(|cause| cause.same_primary(&first_control)));
            let diagnostic =
                super::super::super::exit_diagnostic::BackendCustodyExitDiagnostic::first(
                    first.as_ref(),
                );
            assert_eq!(diagnostic.operation(), stage);
            assert_eq!(returned.win32_code(), Some(109));
        }
        Ok(())
    }

    #[test]
    fn callback_already_recorded_first_cause_is_not_rewrapped_or_replaced()
    -> Result<(), Box<dyn Error>> {
        let original = ClaimError::new(
            "profile_custody_changed_record",
            std::io::Error::from_raw_os_error(13),
        );
        let pointer = original.source().map(std::ptr::from_ref);
        let mut first = Some(original.clone());
        let mut failed = true;
        let returned = retain::<()>(
            Err(NativeObservationError::new("custody_progress", original.clone())),
            &mut first,
            &mut failed,
        )
        .err()
        .ok_or("Worker failure was lost")?;
        assert_eq!(returned.stage(), "custody_progress");
        let retained = first.as_ref().ok_or("Existing source first cause was lost")?;
        assert!(retained.same_primary(&original));
        assert_eq!(retained.source().map(std::ptr::from_ref), pointer);
        assert_eq!(retained.win32_code(), Some(13));
        Ok(())
    }

    #[test]
    fn successful_callback_does_not_create_failure_or_change_existing_source_state() {
        let mut first = None;
        let mut failed = false;
        assert_eq!(retain(Ok(17), &mut first, &mut failed).ok(), Some(17));
        assert!(first.is_none());
        assert!(!failed);
    }
}
