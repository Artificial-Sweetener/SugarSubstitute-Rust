// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove once-only issue, immutable original context and lifetime replay high-water.
use super::{super::*, support::*};
use sugarsubstitute_contracts::incident::*;
fn reply() -> DiagnosticReply {
    DiagnosticReply::Completed { bytes: 2, sha256: [1; 32], revision: None }
}
#[test]
fn exact_pending_duplicate_does_not_issue_again_and_changed_one_rejects() -> TestResult {
    let mut router = DiagnosticRequestRouter::new(key(DiagnosticRole::Storage)?);
    let bytes = encoded(storage(1, DiagnosticCommand::Load)?)?;
    let token = issued(&mut router, &bytes)?;
    assert!(matches!(router.admit(&bytes)?, DiagnosticAdmission::Pending));
    let changed = encoded(storage(1, DiagnosticCommand::Seed(b"{}"))?)?;
    assert_eq!(router.admit(&changed).err(), Some(DiagnosticWireError::ChangedDuplicate));
    router.complete(&token, reply())?;
    assert!(matches!(router.admit(&bytes)?,DiagnosticAdmission::Replay(saved) if saved==reply()));
    Ok(())
}
#[test]
fn evicted_request_retains_high_water_and_never_reexecutes() -> TestResult {
    let mut router = DiagnosticRequestRouter::new(key(DiagnosticRole::Storage)?);
    for id in 1..=9 {
        let bytes = encoded(storage(id, DiagnosticCommand::Load)?)?;
        let token = issued(&mut router, &bytes)?;
        router.complete(&token, reply())?;
    }
    assert_eq!(router.high_water(), 9);
    assert_eq!(
        router.admit(&encoded(storage(1, DiagnosticCommand::Load)?)?).err(),
        Some(DiagnosticWireError::RetiredOperation)
    );
    assert!(matches!(
        router.admit(&encoded(storage(2, DiagnosticCommand::Load)?)?)?,
        DiagnosticAdmission::Replay(_)
    ));
    Ok(())
}
#[test]
fn busy_foreign_malformed_and_stale_completion_preserve_original_pending() -> TestResult {
    let mut router = DiagnosticRequestRouter::new(key(DiagnosticRole::Storage)?);
    let bytes = encoded(storage(1, DiagnosticCommand::Load)?)?;
    let token = issued(&mut router, &bytes)?;
    assert_eq!(
        router.admit(&encoded(storage(2, DiagnosticCommand::Load)?)?).err(),
        Some(DiagnosticWireError::Busy)
    );
    assert!(router.admit(b"garbage").is_err());
    assert_eq!(router.high_water(), 1);
    let other_key =
        DiagnosticKey::new(owner(1)?, DiagnosticRole::Storage, DiagnosticSessionId::new(99)?);
    let mut other = DiagnosticRequestRouter::new(other_key);
    let mut f = storage(1, DiagnosticCommand::Load)?;
    f.key = other_key;
    let foreign = issued(&mut other, &encoded(f)?)?;
    assert_eq!(router.complete(&foreign, reply()), Err(DiagnosticWireError::Stale));
    router.complete(&token, reply())?;
    assert_eq!(router.complete(&token, reply()), Err(DiagnosticWireError::Stale));
    Ok(())
}
#[test]
fn dropping_issued_token_does_not_clear_pending_custody() -> TestResult {
    let mut router = DiagnosticRequestRouter::new(key(DiagnosticRole::Storage)?);
    let bytes = encoded(storage(1, DiagnosticCommand::Load)?)?;
    {
        let _issued = issued(&mut router, &bytes)?;
    }
    assert!(matches!(router.admit(&bytes)?, DiagnosticAdmission::Pending));
    assert_eq!(
        router.admit(&encoded(storage(2, DiagnosticCommand::Load)?)?).err(),
        Some(DiagnosticWireError::Busy)
    );
    Ok(())
}
#[test]
fn changed_original_deadline_utc_operation_and_foreign_key_do_not_advance() -> TestResult {
    let mut router = DiagnosticRequestRouter::new(key(DiagnosticRole::Storage)?);
    let f = storage(1, DiagnosticCommand::Load)?;
    let token = issued(&mut router, &encoded(f)?)?;
    router.complete(&token, reply())?;
    for change in 0..3 {
        let mut f = storage(2, DiagnosticCommand::Load)?;
        match change {
            0 => f.context.deadline = Some(DiagnosticDeadline::new(101, 10)?),
            1 => f.context.at = Some(IncidentUtc::from_unix_seconds(21)?),
            _ => {
                f.context.operation =
                    Some(LogicalStoreOperationKey::new(owner(1)?, IncidentOperationId::new(7)?))
            }
        };
        assert_eq!(router.admit(&encoded(f)?).err(), Some(DiagnosticWireError::Time));
        assert_eq!(router.high_water(), 1)
    }
    let mut f = storage(2, DiagnosticCommand::Load)?;
    f.key = DiagnosticKey::new(owner(1)?, DiagnosticRole::Storage, DiagnosticSessionId::new(99)?);
    assert_eq!(router.admit(&encoded(f)?).err(), Some(DiagnosticWireError::Identity));
    assert_eq!(router.high_water(), 1);
    Ok(())
}
#[test]
fn result_capacity_rejection_keeps_original_pending_and_saved_failure_is_exact() -> TestResult {
    let mut router = DiagnosticRequestRouter::new(key(DiagnosticRole::Storage)?);
    let bytes = encoded(storage(1, DiagnosticCommand::Load)?)?;
    let token = issued(&mut router, &bytes)?;
    assert_eq!(
        router.complete(
            &token,
            DiagnosticReply::Completed { bytes: u64::MAX, sha256: [0; 32], revision: None }
        ),
        Err(DiagnosticWireError::Capacity)
    );
    let failure = DiagnosticReply::Rejected(DiagnosticWireError::Integrity);
    router.complete(&token, failure)?;
    assert!(matches!(router.admit(&bytes)?,DiagnosticAdmission::Replay(saved) if saved==failure));
    Ok(())
}
#[test]
fn maximum_request_id_is_usable_once_without_wrap_or_lifetime_eight_cap() -> TestResult {
    let mut router = DiagnosticRequestRouter::new(key(DiagnosticRole::Storage)?);
    let bytes = encoded(storage(u64::MAX, DiagnosticCommand::Load)?)?;
    let token = issued(&mut router, &bytes)?;
    router.complete(&token, reply())?;
    assert_eq!(router.high_water(), u64::MAX);
    assert_eq!(DiagnosticRequestId::new(0), Err(DiagnosticWireError::Identity));
    assert_eq!(
        router.admit(&encoded(storage(1, DiagnosticCommand::Load)?)?).err(),
        Some(DiagnosticWireError::RetiredOperation)
    );
    Ok(())
}
#[test]
fn parent_operation_fence_survives_helper_session_rotation_without_rebasing() -> TestResult {
    let owner = owner(1)?;
    let mut fence = DiagnosticOperationFence::new(owner);
    let first = LogicalStoreOperationKey::new(owner, IncidentOperationId::new(8)?);
    fence.admit_new(first)?;
    assert_eq!(fence.admit_new(first), Err(DiagnosticWireError::RetiredOperation));
    let historical =
        LogicalStoreOperationKey::new(super::support::owner(2)?, IncidentOperationId::new(9)?);
    assert_eq!(fence.admit_new(historical), Err(DiagnosticWireError::Identity));
    assert_eq!(fence.high_water(), 8);
    fence.admit_new(LogicalStoreOperationKey::new(owner, IncidentOperationId::new(9)?))?;
    assert_eq!(fence.high_water(), 9);
    Ok(())
}

#[test]
fn result_envelope_requires_exact_issued_kind_digest_and_full_context() -> TestResult {
    use sha2::{Digest, Sha256};
    let original = storage(1, DiagnosticCommand::Load)?;
    let bytes = encoded(original)?;
    let mut router = DiagnosticRequestRouter::new(original.key);
    let issued = issued(&mut router, &bytes)?;
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    let kind = DiagnosticRequestKind::of(original.command)?;
    let result = DiagnosticFrame {
        command: DiagnosticCommand::Result { kind, request_sha256: digest, reply: reply() },
        ..original
    };
    assert_eq!(decode(&encoded(result)?)?, result);
    for change in 0..4 {
        let mut bad = result;
        match change {
            0 => bad.request = DiagnosticRequestId::new(2)?,
            1 => bad.context.at = Some(IncidentUtc::from_unix_seconds(21)?),
            2 => {
                bad.command = DiagnosticCommand::Result {
                    kind: DiagnosticRequestKind::of(DiagnosticCommand::Seed(b"{}"))?,
                    request_sha256: digest,
                    reply: reply(),
                }
            }
            _ => {
                bad.command =
                    DiagnosticCommand::Result { kind, request_sha256: [0; 32], reply: reply() }
            }
        };
        assert_eq!(router.accept_result(&issued, bad), Err(DiagnosticWireError::Stale));
    }
    router.accept_result(&issued, result)?;
    assert!(matches!(router.admit(&bytes)?,DiagnosticAdmission::Replay(saved) if saved==reply()));
    Ok(())
}
#[test]
fn unsolicited_result_never_becomes_a_request_or_reopens_completed_work() -> TestResult {
    let original = storage(1, DiagnosticCommand::Load)?;
    let result = DiagnosticFrame {
        command: DiagnosticCommand::Result {
            kind: DiagnosticRequestKind::of(original.command)?,
            request_sha256: [1; 32],
            reply: DiagnosticReply::Failed(DiagnosticOperationFailure::Retirement),
        },
        ..original
    };
    let mut router = DiagnosticRequestRouter::new(original.key);
    assert_eq!(router.admit(&encoded(result)?).err(), Some(DiagnosticWireError::Schema));
    assert_eq!(router.high_water(), 0);
    Ok(())
}
#[test]
fn all_closed_operation_failure_results_roundtrip_without_settlement() -> TestResult {
    for failure in [
        DiagnosticOperationFailure::Admission,
        DiagnosticOperationFailure::Cancelled,
        DiagnosticOperationFailure::Deadline,
        DiagnosticOperationFailure::Capacity,
        DiagnosticOperationFailure::Identity,
        DiagnosticOperationFailure::Storage,
        DiagnosticOperationFailure::Recovery,
        DiagnosticOperationFailure::Capture,
        DiagnosticOperationFailure::Symbols,
        DiagnosticOperationFailure::Export,
        DiagnosticOperationFailure::Presentation,
        DiagnosticOperationFailure::Retirement,
    ] {
        let original = storage(1, DiagnosticCommand::Load)?;
        let f = DiagnosticFrame {
            command: DiagnosticCommand::Result {
                kind: DiagnosticRequestKind::of(original.command)?,
                request_sha256: [1; 32],
                reply: DiagnosticReply::Failed(failure),
            },
            ..original
        };
        assert_eq!(decode(&encoded(f)?)?, f)
    }
    Ok(())
}

#[test]
fn storage_retirement_cannot_renew_the_original_operation_deadline() -> TestResult {
    let original = storage(1, DiagnosticCommand::Load)?;
    let mut router = DiagnosticRequestRouter::new(original.key);
    let bytes = encoded(original)?;
    let token = issued(&mut router, &bytes)?;
    router.complete(&token, DiagnosticReply::Rejected(DiagnosticWireError::Integrity))?;
    let mut retire = original;
    retire.request = DiagnosticRequestId::new(2)?;
    retire.command = DiagnosticCommand::Retire;
    retire.context = DiagnosticContext {
        deadline: Some(DiagnosticDeadline::new(101, 10)?),
        ..DiagnosticContext::default()
    };
    assert_eq!(router.admit(&encoded(retire)?).err(), Some(DiagnosticWireError::Time));
    assert_eq!(router.high_water(), 1);
    retire.context.deadline = original.context.deadline;
    let _token = issued(&mut router, &encoded(retire)?)?;
    assert_eq!(router.high_water(), 2);
    Ok(())
}

#[test]
fn changing_restart_eligibility_cannot_reuse_an_original_authenticated_view_request() -> TestResult
{
    let session = binding()?.session();
    let frame = reporter(
        1,
        DiagnosticCommand::BeginView {
            session,
            restart: ReportRestartAvailability::Unavailable,
            total: 1,
            sha256: [1; 32],
        },
        true,
    )?;
    let mut router = DiagnosticRequestRouter::new(frame.key);
    let bytes = encoded(frame)?;
    let token = issued(&mut router, &bytes)?;
    assert!(matches!(router.admit(&bytes)?, DiagnosticAdmission::Pending));
    let changed = DiagnosticFrame {
        command: DiagnosticCommand::BeginView {
            session,
            restart: ReportRestartAvailability::Available,
            total: 1,
            sha256: [1; 32],
        },
        ..frame
    };
    assert_eq!(router.admit(&encoded(changed)?).err(), Some(DiagnosticWireError::ChangedDuplicate));
    assert_eq!(router.high_water(), 1);
    router.complete(&token, reply())?;
    assert!(matches!(router.admit(&bytes)?, DiagnosticAdmission::Replay(_)));
    Ok(())
}
