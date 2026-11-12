// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check actual terminal custody extraction without claiming parent/native retirement authority.
use super::*;
pub(super) fn extract(
    case: Case,
    relay: &mut ReporterRelay,
    until: Moment,
    expected: Option<TerminalKind>,
) -> TestResult<Option<failure::ReporterSecondaryFailure>> {
    let retained = relay.work.take_failure(relay.matched);
    if case == Case::Mismatched {
        assert!(retained.is_none());
        assert!(matches!(relay.work, RelayWork::Delivered(_)));
    } else {
        match retained.as_ref().ok_or("Actual failed lifecycle custody missing")? {
            failure::ReporterSecondaryFailure::Worker { _completion: done } => {
                assert!(matches!(case, Case::Deferred | Case::Cancelled | Case::Expired));
                assert_eq!(done.spec.deadline, Some(until));
                assert_eq!(
                    done.outcome.kind(),
                    if case == Case::Deferred {
                        TerminalKind::Failure
                    } else {
                        expected.ok_or("Actual terminal missing")?
                    }
                );
            }
            failure::ReporterSecondaryFailure::Admission { _cause: cause } => {
                assert!(matches!(case, Case::Rejected | Case::Capacity | Case::RetryExpired));
                assert!(matches!(
                    cause,
                    AdmissionError::ScopeClosed { .. } | AdmissionError::DeadlineExpired { .. }
                ));
            }
        }
        assert!(matches!(relay.work, RelayWork::Idle));
    }
    Ok(retained)
}
