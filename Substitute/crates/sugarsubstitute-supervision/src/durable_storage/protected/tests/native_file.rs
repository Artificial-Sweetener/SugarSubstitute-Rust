// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve adopted output custody when a later native query reports missing.
use super::*;
use crate::{
    native_claims::files::Budget,
    durable_storage::protected::native_file::{FileGrant, NativeFile, NativeFileAdmission},
    durable_storage::protected::native_name::NativeName,
};
use windows::Win32::{
    Foundation::{ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND},
    System::Threading::CreateEventW,
};

fn missing() -> ClaimError {
    ClaimError::new(
        "fixture_late_query",
        windows::core::Error::from_hresult(ERROR_FILE_NOT_FOUND.to_hresult()),
    )
}

#[test]
fn native_missing_without_output_classifies_only_actual_empty_cell() -> TestResult {
    admitted(|context| {
        let memory = Arc::new(
            context.memory().claim(STORAGE_BYTES).map_err(|e| ClaimError::new("test_claim", e))?,
        );
        let mut file = NativeFile::reserve(
            1,
            0,
            FileGrant::MetadataReadOnly,
            &mut Some((
                NativeName::Ordinary(PathBuf::from("absence-shape")),
                NativeFileAdmission::Ordinary { budget: Budget::Protected(memory) },
            )),
        )?;
        assert!(file.missing_without_output(&missing()));
        let denied = ClaimError::new(
            "fixture_open",
            windows::core::Error::from_hresult(ERROR_ACCESS_DENIED.to_hresult()),
        );
        assert!(!file.missing_without_output(&denied));
        file.close()?;
        Ok(())
    })
}

#[test]
fn native_late_missing_query_retains_actual_adopted_output_and_cause() -> TestResult {
    admitted(|context| {
        use std::error::Error;
        let memory = Arc::new(
            context.memory().claim(STORAGE_BYTES).map_err(|e| ClaimError::new("test_claim", e))?,
        );
        let mut file = NativeFile::reserve(
            1,
            0,
            FileGrant::MetadataReadOnly,
            &mut Some((
                NativeName::Ordinary(PathBuf::from("late-query-shape")),
                NativeFileAdmission::Ordinary { budget: Budget::Protected(memory) },
            )),
        )?;
        // SAFETY: The unnamed fixture event has independent ownership and no callback.
        let raw = unsafe { CreateEventW(None, false, false, None) }
            .map_err(|e| ClaimError::new("fixture_event", e))?;
        file.fields_mut()?.handle.adopt_output(raw);
        let cause = missing();
        let classified = file.missing_without_output(&cause);
        let retained = file.fields()?.handle.is_armed() && file.fields()?.handle.raw() == raw;
        let code = cause
            .source()
            .and_then(|e| e.downcast_ref::<windows::core::Error>())
            .map(windows::core::Error::code);
        // Checked retirement precedes assertions so an assertion cannot abandon this fixture.
        file.close()?;
        assert!(!classified);
        assert!(retained);
        assert_eq!(code, Some(ERROR_FILE_NOT_FOUND.to_hresult()));
        assert!(file.missing_without_output(&cause));
        Ok(())
    })
}
