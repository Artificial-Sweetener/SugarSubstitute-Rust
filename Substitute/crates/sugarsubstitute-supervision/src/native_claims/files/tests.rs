// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real admitted output credits and physical close/borrow custody.
use super::test_support::{TestResult, admitted};
use super::*;
use sugarsubstitute_contracts::execution::{JobContext, NativeHandleClaim, SymbolNativePartition};
use windows::Win32::{Foundation::GetHandleInformation, System::Threading::CreateEventW};

fn claim(context: &JobContext) -> Result<NativeHandleClaim, ClaimError> {
    let lease = context.native_handles().ok_or_else(|| ClaimError::invalid("test_real_lease"))?;
    let claim = lease
        .take_partition(SymbolNativePartition::SelectedRelease)
        .map_err(|e| ClaimError::new("test_partition", e))?;
    if !claim.matches_lease(lease) {
        return Err(ClaimError::invalid("test_original_ledger"));
    }
    Ok(claim)
}
fn reserved(context: &JobContext, claim: &NativeHandleClaim) -> Result<Handle, ClaimError> {
    let memory =
        Arc::new(context.memory().claim(1024).map_err(|e| ClaimError::new("test_memory", e))?);
    Ok(Handle::reserve_output(
        Budget::Protected(memory),
        claim.reserve_output().map_err(|e| ClaimError::new("test_output", e))?,
    ))
}
fn event() -> Result<HANDLE, ClaimError> {
    // SAFETY: This unnamed fixture output has no callback, inheritance or shared owner.
    unsafe { CreateEventW(None, false, false, None) }.map_err(|e| ClaimError::new("test_event", e))
}

#[test]
fn counted_uninvoked_and_actual_no_output_return_only_their_original_credits() -> TestResult {
    admitted(|context| {
        let claim = claim(&context)?;
        let mut uninvoked = reserved(&context, &claim)?;
        assert_eq!(claim.in_custody(), 1);
        uninvoked.close()?;
        assert_eq!(claim.in_custody(), 0);
        let mut empty = reserved(&context, &claim)?;
        empty.adopt_output(INVALID_HANDLE_VALUE);
        empty.release_unused_output()?;
        assert_eq!(claim.in_custody(), 0);
        assert!(!empty.is_armed());
        Ok(())
    })
}
#[test]
fn counted_actual_adoption_rejects_unused_release_until_checked_os_close() -> TestResult {
    admitted(|context| {
        let claim = claim(&context)?;
        let mut file = reserved(&context, &claim)?;
        let raw = event()?;
        file.adopt_output(raw);
        let rejected = file.release_unused_output().is_err();
        let before = (file.is_armed(), claim.in_custody());
        file.close()?;
        assert!(rejected);
        assert_eq!(before, (true, 1));
        assert_eq!(claim.in_custody(), 0);
        assert!(!file.is_armed());
        Ok(())
    })
}
#[test]
fn counted_borrow_unwind_restores_same_raw_and_keeps_original_credit() -> TestResult {
    admitted(|context| {
        let claim = claim(&context)?;
        let mut file = reserved(&context, &claim)?;
        let raw = event()?;
        file.adopt_output(raw);
        let unwind =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), ClaimError> {
                let borrowed = file.borrow_file()?;
                assert!(borrowed.parent.raw().0.is_null());
                assert!(borrowed.parent.is_armed());
                assert_eq!(claim.in_custody(), 1);
                std::panic::resume_unwind(Box::new("fixture borrow unwind"));
            }));
        let restored = file.raw() == raw && file.is_armed() && claim.in_custody() == 1;
        file.close()?;
        assert!(unwind.is_err());
        assert!(restored);
        assert_eq!(claim.in_custody(), 0);
        Ok(())
    })
}
#[test]
fn counted_checked_close_failure_and_drop_preserve_first_cause_memory_and_credit() -> TestResult {
    admitted(|context| {
        use std::error::Error;
        let claim = claim(&context)?;
        let mut file = reserved(&context, &claim)?;
        let raw = event()?;
        file.adopt_output(raw);
        let mut exact = false;
        let result = file.close_using(|actual| {
            exact = actual == raw;
            Err(ClaimError::new(
                "close_handle",
                windows::core::Error::from_hresult(
                    windows::Win32::Foundation::ERROR_INVALID_HANDLE.to_hresult(),
                ),
            ))
        });
        let first_operation = file.first_failure.as_ref().map(ClaimError::operation);
        let observed_code = result.as_ref().err().and_then(|cause| {
            cause
                .source()
                .and_then(|e| e.downcast_ref::<windows::core::Error>())
                .map(windows::core::Error::code)
        });
        let retained = file.raw() == raw && file.is_armed() && claim.in_custody() == 1;
        drop(file);
        // SAFETY: Test-only retirement of its still-live exact output, after production ownership is retained.
        unsafe { CloseHandle(raw) }.map_err(|e| ClaimError::new("test_fixture_close", e))?;
        assert!(exact);
        assert!(retained);
        assert_eq!(first_operation, Some("close_handle"));
        assert_eq!(
            observed_code,
            Some(windows::Win32::Foundation::ERROR_INVALID_HANDLE.to_hresult())
        );
        assert_eq!(claim.in_custody(), 1);
        assert!(context.memory().claim(context.memory().bytes()).is_err());
        Ok(())
    })
}
#[test]
fn counted_abandoned_actual_output_remains_live_after_owner_drop() -> TestResult {
    admitted(|context| {
        let claim = claim(&context)?;
        let mut file = reserved(&context, &claim)?;
        let raw = event()?;
        file.adopt_output(raw);
        drop(file);
        let mut flags = 0;
        // SAFETY: The fixture observes the exact retained output without transferring ownership.
        let live = unsafe { GetHandleInformation(raw, &mut flags) };
        // SAFETY: Explicit fixture cleanup closes its known output; production credit stays uncertain.
        let cleanup =
            unsafe { CloseHandle(raw) }.map_err(|e| ClaimError::new("test_fixture_close", e));
        cleanup?;
        assert!(live.is_ok());
        assert_eq!(claim.in_custody(), 1);
        assert!(context.memory().claim(context.memory().bytes()).is_err());
        Ok(())
    })
}
