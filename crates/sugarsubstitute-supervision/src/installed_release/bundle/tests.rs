// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify compiled closed-region construction and last-alias custody on real admission.
use super::*;
use crate::native_claims::files::test_support::{TestResult, capture};
#[test]
fn actual_capture_constructor_fits_closed_regions_and_releases_uninvoked_owner() -> TestResult {
    capture(|admission, context| {
        let origin_bytes = admission.into_origin();
        let (metadata, ingress, scratch) =
            capacity::regions(&origin_bytes).map_err(ClaimError::capture_allocation)?;
        assert!(
            metadata <= METADATA_BYTES,
            "actual compiled metadata {metadata} exceeds {METADATA_BYTES}"
        );
        assert!(
            ingress <= INGRESS_BYTES,
            "actual compiled ingress {ingress} exceeds {INGRESS_BYTES}"
        );
        assert!(
            scratch <= SCRATCH_BYTES,
            "actual compiled scratch {scratch} exceeds {SCRATCH_BYTES}"
        );
        capacity::preflight(&origin_bytes).map_err(ClaimError::capture_allocation)?;
        let original = AllocationOrigin::new(origin_bytes)
            .map_err(|failure| ClaimError::capture_allocation(failure.cause))?;
        let owner = CaptureBundleOwner::construct(original.clone())?;
        assert!(owner.fields.as_ref().is_none_or(|fields| fields.facts.is_none()));
        assert!(context.memory().claim(2_097_153).is_err());
        drop(owner);
        drop(original);
        assert_eq!(
            context
                .memory()
                .claim(2_097_152)
                .map_err(|cause| ClaimError::new("fixture_capture_return", cause))?
                .bytes(),
            2_097_152
        );
        Ok(())
    })
}
#[test]
fn final_capture_cause_alias_keeps_original_charge_after_owner_storage_drop() -> TestResult {
    capture(|admission, context| {
        let original = AllocationOrigin::new(admission.into_origin())
            .map_err(|failure| ClaimError::capture_allocation(failure.cause))?;
        let mut owner = CaptureBundleOwner::construct(original.clone())?;
        let fields = owner.fields.as_mut().ok_or_else(missing)?;
        let cause = fields.causes[0].as_mut().ok_or_else(missing)?.selected_error(
            "fixture_capture_native",
            super::super::CaptureBoundedCause::Status(super::super::causes::CaptureCryptoStatus(
                -17,
            )),
            false,
        );
        let alias = cause.clone();
        drop(cause);
        drop(owner);
        drop(original);
        assert!(context.memory().claim(2_097_153).is_err());
        assert!(std::error::Error::source(&alias).is_some_and(|cause| {
            cause
                .downcast_ref::<super::super::causes::CaptureCryptoStatus>()
                .is_some_and(|cause| cause.0 == -17)
        }));
        drop(alias);
        assert_eq!(
            context
                .memory()
                .claim(2_097_152)
                .map_err(|cause| ClaimError::new("fixture_capture_alias_return", cause))?
                .bytes(),
            2_097_152
        );
        Ok(())
    })
}

#[test]
fn actual_selected_origin_cannot_enter_ordinary_file_or_phase_branch() -> TestResult {
    capture(|admission, _context| {
        let original = AllocationOrigin::new(admission.into_origin())
            .map_err(|failure| ClaimError::capture_allocation(failure.cause))?;
        let budget = crate::native_claims::files::Budget::SelectedRelease(original);
        assert!(crate::protected_storage::native_phase::NativePhase::ordinary(&budget).is_err());
        let result = NativeFile::reserve(
            1,
            1,
            crate::protected_storage::native_file::FileGrant::ReadOnly,
            &mut Some((
                crate::protected_storage::native_name::NativeName::Ordinary(
                    std::path::PathBuf::from(r"E:ixture-uninvoked"),
                ),
                crate::protected_storage::native_file::NativeFileAdmission::Ordinary { budget },
            )),
        );
        let failure = match result {
            Err(failure) => failure,
            Ok(file) => {
                drop(file);
                return Err(missing());
            }
        };
        assert_eq!(failure.operation(), "capture_native_allocation");
        Ok(())
    })
}
#[cfg(not(feature = "local-k1-symbol-trust"))]
#[test]
fn feature_off_rejects_missing_anchor_before_any_native_output() -> TestResult {
    capture(|admission, _context| {
        let mut owner = CaptureBundleOwner::new(
            super::super::InstalledReleaseOwner::new().capture_selection(),
            admission,
        )
        .map_err(|failure| failure.cause)?;
        let failure = owner.select().err().ok_or_else(missing)?;
        assert_eq!(failure.operation(), "missing_trusted_release");
        let fields = owner.fields.as_ref().ok_or_else(missing)?;
        assert!(fields.files.iter().all(Option::is_none));
        assert!(fields.directories.iter().flatten().all(|cell| !cell.handle.is_armed()));
        assert!(!fields.crypto.as_ref().is_some_and(CryptoOwner::has_output));
        Ok(())
    })
}

#[test]
fn rejected_capture_handle_input_keeps_original_credit_until_same_slot_success() -> TestResult {
    capture(|admission, _context| {
        let original = AllocationOrigin::new(admission.into_origin())
            .map_err(|failure| ClaimError::capture_allocation(failure.cause))?;
        let owner = CaptureBundleOwner::construct(original.clone())?;
        let credit = owner
            .fields
            .as_ref()
            .ok_or_else(missing)?
            .quota
            .reserve_output()
            .map_err(ClaimError::capture_allocation)?;
        let selected = original.clone();
        let result = capture(move |foreign, _foreign_context| {
            let foreign = AllocationOrigin::new(foreign.into_origin())
                .map_err(|failure| ClaimError::capture_allocation(failure.cause))?;
            let mut input = Some((foreign.clone(), credit));
            let rejection = Handle::reserve_capture(&mut input).err().ok_or_else(missing)?;
            assert_eq!(rejection.operation(), "capture_native_allocation");
            let (retained, output) = input.as_ref().ok_or_else(missing)?;
            assert!(retained.matches(&foreign));
            assert!(output.original().matches(&selected));
            input.as_mut().ok_or_else(missing)?.0 = selected.clone();
            let mut handle = Handle::reserve_capture(&mut input)?;
            assert!(input.is_none());
            assert!(!handle.is_armed());
            handle.release_unused_output()?;
            drop(handle);
            drop(foreign);
            Ok(())
        });
        assert!(result.is_ok(), "{result:?}");
        drop(owner);
        drop(original);
        Ok(())
    })
}
