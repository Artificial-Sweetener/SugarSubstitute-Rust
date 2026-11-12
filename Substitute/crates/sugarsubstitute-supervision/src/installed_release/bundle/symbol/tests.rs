// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify real paired Symbol admission, closed native credit and final alias custody.
use super::*;
use crate::native_claims::files::test_support::{TestResult, symbol};
use sugarsubstitute_contracts::execution::SymbolNativePartition;
fn owner(admission: SymbolBundleAdmission) -> Result<SymbolBundleOwner, ClaimError> {
    SymbolBundleOwner::new(crate::InstalledReleaseOwner::new().symbol_selection(), admission)
        .map_err(|failure| failure.cause)
}
#[test]
fn actual_symbol_owner_uses_selected_ledger_and_returns_uninvoked_outputs() -> TestResult {
    symbol(|admission, context| {
        let ledger = context.native_handles().ok_or_else(missing)?;
        let observation = ledger.observation();
        let remaining = context
            .memory()
            .claim(6_057_984)
            .map_err(|cause| ClaimError::new("fixture_symbol_remaining", cause))?;
        let owner = owner(admission)?;
        let (metadata, ingress, scratch) =
            capacity::regions(owner.allocation.as_ref().ok_or_else(missing)?.original().original())
                .map_err(ClaimError::capture_allocation)?;
        assert!(metadata <= METADATA_BYTES && ingress <= INGRESS_BYTES && scratch <= SCRATCH_BYTES);
        assert_eq!(observation.snapshot(), (87, 87));
        assert!(ledger.take_partition(SymbolNativePartition::SelectedRelease).is_err());
        assert!(context.memory().claim(1).is_err());
        assert!(
            owner.fields.as_ref().is_some_and(|fields| fields.modules.iter().all(Option::is_none))
        );
        drop(owner);
        assert_eq!(observation.snapshot(), (0, 87));
        let returned = context
            .memory()
            .claim(2_097_152)
            .map_err(|cause| ClaimError::new("fixture_symbol_return", cause))?;
        assert_eq!(returned.bytes(), 2_097_152);
        drop(returned);
        drop(remaining);
        Ok(())
    })
}
#[test]
fn symbol_cause_last_alias_keeps_charge_after_physical_storage_deallocation() -> TestResult {
    symbol(|admission, context| {
        let remaining = context
            .memory()
            .claim(6_057_984)
            .map_err(|cause| ClaimError::new("fixture_symbol_remaining", cause))?;
        let mut owner = owner(admission)?;
        let fields = owner.fields.as_mut().ok_or_else(missing)?;
        let cause = fields.causes[0].as_mut().ok_or_else(missing)?.selected_error(
            "fixture_symbol_native",
            super::super::super::causes::CaptureBoundedCause::Status(
                super::super::super::causes::CaptureCryptoStatus(-31),
            ),
            false,
        );
        let alias = cause.clone();
        drop(cause);
        drop(owner);
        assert!(context.memory().claim(1).is_err());
        assert!(std::error::Error::source(&alias).is_some_and(|cause| {
            cause
                .downcast_ref::<super::super::super::causes::CaptureCryptoStatus>()
                .is_some_and(|status| status.0 == -31)
        }));
        drop(alias);
        let returned = context
            .memory()
            .claim(2_097_152)
            .map_err(|cause| ClaimError::new("fixture_symbol_alias_return", cause))?;
        assert_eq!(returned.bytes(), 2_097_152);
        drop(returned);
        drop(remaining);
        Ok(())
    })
}
#[test]
fn symbol_quota_rejects_uncharged_capture_reservation_and_unverified_lease() -> TestResult {
    symbol(|admission, context| {
        let owner = owner(admission)?;
        let quota = &owner.fields.as_ref().ok_or_else(missing)?.quota;
        assert!(quota.reserve_output().is_err());
        assert_eq!(context.native_handles().ok_or_else(missing)?.observation().snapshot().0, 87);
        assert!(owner.take_verified().is_err());
        Ok(())
    })
}
#[cfg(not(feature = "local-k1-symbol-trust"))]
#[test]
fn symbol_feature_off_missing_anchor_remembers_cause_without_native_effects() -> TestResult {
    symbol(|admission, _| {
        let mut owner = owner(admission)?;
        let first = owner.select().err().ok_or_else(missing)?;
        assert_eq!(first.operation(), "missing_trusted_release");
        let second = owner.select().err().ok_or_else(missing)?;
        assert_eq!(first.operation(), second.operation());
        let fields = owner.fields.as_ref().ok_or_else(missing)?;
        assert!(fields.files.iter().all(Option::is_none));
        assert!(fields.directories.iter().flatten().all(|cell| !cell.handle.is_armed()));
        assert!(!fields.crypto.as_ref().is_some_and(CryptoOwner::has_output));
        assert!(owner.take_verified().is_err());
        Ok(())
    })
}
