// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep completion evidence scoped to one quiescent operation, before native preparation.
use super::*;
use crate::runtime_test_support::{Rig, TestResult};
#[test]
fn previous_completion_cannot_survive_next_preparation_failure_or_live_reuse() -> TestResult {
    let rig = Rig::new()?;
    let mut operation = Operation::new(Signal::new(&rig.client.core));
    operation.completed = Some(4);
    operation.begin_reuse()?;
    // Any following CreateEvent/ResetEvent failure sees no previous successful count.
    assert_eq!(operation.take_completed(), None);
    operation.completed = Some(4);
    assert_eq!(operation.take_completed(), Some(4));
    assert_eq!(operation.take_completed(), None);
    operation.pending = true;
    operation.completed = Some(3);
    assert!(operation.begin_reuse().is_err());
    assert_eq!(operation.completed, Some(3));
    assert_eq!(operation.take_completed(), None);
    Ok(())
}

#[test]
fn native_acceptance_adopts_pending_custody_before_completion_polling() -> TestResult {
    let rig = Rig::new()?;
    for synchronous in [false, true] {
        let mut operation = Operation::new(Signal::new(&rig.client.core));
        let result = if synchronous {
            Ok(())
        } else {
            Err(windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(
                ERROR_IO_PENDING.0,
            )))
        };
        let accepted = operation.accept(result, false)?;
        assert!(operation.pending);
        assert_eq!(operation.completed, None);
        assert_eq!(operation.take_completed(), None);
        assert!(operation.quiesce().is_err());
        assert!(matches!(accepted, AcceptedIssue::Synchronous) == synchronous);
        // This CPU boundary test classifies acceptance only; no native poll or close.
        operation.pending = false;
    }
    let mut rejected = Operation::new(Signal::new(&rig.client.core));
    assert!(
        rejected
            .accept(
                Err(windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(
                    ERROR_ACCESS_DENIED.0
                ))),
                false
            )
            .is_err()
    );
    assert!(!rejected.pending);
    assert_eq!(rejected.take_completed(), None);
    Ok(())
}
