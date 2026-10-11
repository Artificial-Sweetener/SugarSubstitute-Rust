// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify pure invocation/codec/evidence decisions without claiming native authentication.
use super::*;
use std::{
    ffi::OsString,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{
    execution::Moment,
    lifecycle::{
        ConnectionId, CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, ProcessId,
        ProcessKey, RequestId, RunGeneration, RunKey, ShutdownId,
    },
};
use sugarsubstitute_supervision::{
    RegistryObservation,
    lifecycle_protocol::{CapabilityRef, DecodedPayload, FrameCodec, WirePayload},
};

fn run() -> Result<RunKey> {
    Ok(RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([1; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    })
}
fn arguments() -> Vec<OsString> {
    ["--section", "64", "--deadline-qpc", "100", "--qpc-frequency", "10", "--case", "valid"]
        .map(OsString::from)
        .to_vec()
}

#[test]
fn all_three_fixed_roles_parse_without_native_adoption() -> Result<()> {
    for kind in [
        options::CaseKind::Valid,
        options::CaseKind::WrongCapability,
        options::CaseKind::ForeignGeneration,
    ] {
        let mut input = arguments();
        input[7] = kind.name().into();
        let parsed = ChildOptions::parse(input)?;
        assert_eq!(parsed.kind, kind);
        assert_eq!(parsed.deadline.ticks(), 100);
        assert_eq!(parsed.deadline.frequency(), 10);
        assert!(parsed.section.is_some());
    }
    Ok(())
}

#[test]
fn role_parser_rejects_missing_duplicate_reordered_and_extra_input() {
    for index in 0..arguments().len() {
        let mut input = arguments();
        input.remove(index);
        assert!(ChildOptions::parse(input).is_err());
    }
    let mut duplicate = arguments();
    duplicate[2] = "--section".into();
    let mut reordered = arguments();
    reordered.swap(0, 2);
    let mut extra = arguments();
    extra.push("--receipt".into());
    for input in [duplicate, reordered, extra] {
        assert!(ChildOptions::parse(input).is_err());
    }
}

#[test]
fn role_numbers_reject_noncanonical_nonascii_and_overflow_before_bind() {
    for index in [1, 3, 5] {
        for value in [
            "0",
            "00",
            "01",
            "+1",
            "-1",
            " 1",
            "1 ",
            "１",
            "18446744073709551616",
            "999999999999999999999",
        ] {
            let mut input = arguments();
            input[index] = value.into();
            assert!(ChildOptions::parse(input).is_err());
        }
    }
    for index in [3, 5] {
        let mut input = arguments();
        input[index] = u64::MAX.to_string().into();
        assert!(ChildOptions::parse(input).is_err());
    }
}

#[test]
fn role_parser_rejects_pseudohandles_and_unknown_or_nonunicode_mode() {
    use std::os::windows::ffi::OsStringExt;
    for value in [u64::MAX, u64::MAX - 1, u64::MAX - 15] {
        let mut input = arguments();
        input[1] = value.to_string().into();
        assert!(ChildOptions::parse(input).is_err());
    }
    for value in [OsString::from("unknown"), OsString::from_wide(&[0xd800])] {
        let mut input = arguments();
        input[7] = value;
        assert!(ChildOptions::parse(input).is_err());
    }
}

#[test]
fn wire_header_binding_rejects_each_claimed_identity_or_deadline_change() -> Result<()> {
    let expected =
        protocol::header(run()?, Some(ConnectionId::new(1)?), Some(RequestId::new(1)?), 100);
    protocol::validate(expected, expected)?;
    let mut changed = [expected; 6];
    changed[0].run.generation = RunGeneration::new(2)?;
    changed[1].run.instance = InstanceId::new(2)?;
    changed[2].run.owner.epoch = CoordinatorEpoch::new([2; 16])?;
    changed[3].connection = Some(ConnectionId::new(2)?);
    changed[4].request = Some(RequestId::new(2)?);
    changed[5].deadline_qpc = 101;
    for actual in changed {
        assert!(protocol::validate(actual, expected).is_err());
    }
    Ok(())
}

#[test]
fn codec_hello_keeps_secret_out_of_debug_and_validates_whole_borrowed_frame() -> Result<()> {
    let candidate = [0xab; 32];
    let message = protocol::message(
        protocol::header(run()?, None, None, 100),
        WirePayload::Hello(CapabilityRef::new(&candidate)),
    );
    let mut storage = [0; 256];
    let bytes = FrameCodec::encode(&message, &mut storage)?;
    let decoded = FrameCodec::decode(&storage[..bytes])?;
    let DecodedPayload::Hello(capability) = decoded.payload else {
        return Err("Unexpected codec fixture".into());
    };
    assert_eq!(capability.bytes(), &candidate);
    assert!(!format!("{message:?} {capability:?}").contains("171"));
    assert!(FrameCodec::decode(&storage[..bytes + 1]).is_err());
    Ok(())
}

#[test]
fn negative_disconnect_is_exact_native_stage_code_without_fatal_facts() {
    for code in [109, 233] {
        assert!(failure::disconnect_facts("pipe_completion", Some(code), false, false));
    }
    for (stage, code, unrestored, panicked) in [
        ("pipe_issue", Some(109), false, false),
        ("pipe_completion", Some(5), false, false),
        ("pipe_completion", None, false, false),
        ("pipe_completion", Some(109), true, false),
        ("pipe_completion", Some(233), false, true),
    ] {
        assert!(!failure::disconnect_facts(stage, code, unrestored, panicked));
    }
}

#[test]
fn stop_uses_canonical_ten_fifteen_budget_once_and_late_cleanup_is_not_success() -> Result<()> {
    let native = Instant::now();
    let mut stop = evidence::Stop::new(ShutdownId::new(1)?, Moment::from_nanos(0), native)?;
    assert_eq!(stop.next_deadline(), native.checked_add(Duration::from_secs(10)));
    assert_eq!(stop.due(Moment::from_nanos(9_999_999_999), false), (false, false));
    assert_eq!(stop.due(Moment::from_nanos(10_000_000_000), false), (true, false));
    assert_eq!(stop.next_deadline(), native.checked_add(Duration::from_secs(15)));
    assert_eq!(stop.due(Moment::from_nanos(15_000_000_000), false), (true, true));
    assert_eq!(stop.due(Moment::from_nanos(20_000_000_000), true), (false, false));
    assert_eq!(stop.next_deadline(), None);
    assert_eq!(stop.budget.settle_until.as_nanos(), 15_000_000_000);
    Ok(())
}

#[test]
fn matching_unexpected_exit_fact_survives_error_for_owned_cleanup() -> Result<()> {
    let mut case = evidence::Case::new(options::CaseKind::Valid, run()?);
    let process = ProcessKey { owner: case.run.owner, id: ProcessId::new(1)? };
    case.process = Some(process);
    assert!(case.observe_registry(RegistryObservation::RootExited { process, code: 259 }).is_err());
    assert_eq!(case.exited, Some(259));
    assert!(!case.settled);
    assert!(!case.complete());
    Ok(())
}

#[test]
fn foreign_family_fact_cannot_change_current_exit_or_settlement() -> Result<()> {
    let mut case = evidence::Case::new(options::CaseKind::WrongCapability, run()?);
    let process = ProcessKey { owner: case.run.owner, id: ProcessId::new(1)? };
    case.process = Some(process);
    let foreign = ProcessKey { id: ProcessId::new(2)?, ..process };
    assert!(
        case.observe_registry(RegistryObservation::RootExited { process: foreign, code: 37 })
            .is_err()
    );
    assert_eq!(case.exited, None);
    assert!(
        case.observe_registry(RegistryObservation::FamilySettled {
            process,
            shutdown: Some(ShutdownId::new(1)?)
        })
        .is_err()
    );
    assert!(!case.settled);
    Ok(())
}

#[test]
fn denial_and_pipe_terminal_do_not_manufacture_process_family_settlement() -> Result<()> {
    let mut case = evidence::Case::new(options::CaseKind::ForeignGeneration, run()?);
    case.denied = true;
    case.prepared = true;
    case.connected = true;
    case.hello = true;
    case.pipe_terminal = true;
    case.started = true;
    case.exited = Some(37);
    assert!(!case.complete());
    case.settled = true;
    assert!(!case.complete());
    case.retired = true;
    assert!(case.complete());
    case.register = true;
    assert!(!case.complete());
    Ok(())
}

#[test]
fn exact_terminal_notice_survives_observation_error_and_empty_closing_activity() -> Result<()> {
    use sugarsubstitute_contracts::execution::{
        CompletionBatch, CompletionNotification, JobId, ScopeId, TerminalKind,
    };
    let scope = ScopeId::new(1)?;
    let job = JobId::new(2)?;
    let batch = |kind| CompletionBatch {
        notifications: vec![CompletionNotification {
            scope_id: scope,
            job_id: job,
            kind,
            replacement: None,
        }],
        remaining: 0,
    };
    let mut pending = terminal::PendingTerminal::default();
    pending.capture(scope, job, &batch(TerminalKind::Failure))?;
    // The codec error and empty next wake must not discard the delivered metadata.
    assert!(
        protocol::validate(
            protocol::header(run()?, None, None, 1),
            protocol::header(run()?, None, None, 2)
        )
        .is_err()
    );
    pending.capture(scope, job, &CompletionBatch { notifications: vec![], remaining: 0 })?;
    assert_eq!(pending.kind(), Some(TerminalKind::Failure));
    assert!(pending.capture(scope, job, &batch(TerminalKind::Cancelled)).is_err());
    assert_eq!(pending.kind(), Some(TerminalKind::Failure));
    Ok(())
}

#[test]
fn foreign_terminal_notice_cannot_authorize_current_typed_transfer() -> Result<()> {
    use sugarsubstitute_contracts::execution::{
        CompletionBatch, CompletionNotification, JobId, ScopeId, TerminalKind,
    };
    let mut pending = terminal::PendingTerminal::default();
    let scope = ScopeId::new(1)?;
    let job = JobId::new(2)?;
    let batch = CompletionBatch {
        notifications: vec![CompletionNotification {
            scope_id: ScopeId::new(2)?,
            job_id: job,
            kind: TerminalKind::Cancelled,
            replacement: None,
        }],
        remaining: 0,
    };
    pending.capture(scope, job, &batch)?;
    assert_eq!(pending.kind(), None);
    Ok(())
}

#[test]
fn unexpected_terminal_classes_remain_distinct_and_panic_text_stays_redacted() -> Result<()> {
    use failure::TerminalFailure;
    use sugarsubstitute_contracts::execution::{JobId, JobOutcome, PanicCause};
    assert!(matches!(failure::unexpected(JobOutcome::Cancelled), TerminalFailure::Cancelled));
    assert!(matches!(failure::unexpected(JobOutcome::TimedOut), TerminalFailure::TimedOut));
    let replacement = JobId::new(3)?;
    assert!(matches!(failure::unexpected(JobOutcome::Coalesced { replacement }),
                    TerminalFailure::Coalesced(actual) if actual == replacement));
    let error =
        failure::unexpected(JobOutcome::Panicked(PanicCause::new("sensitive cause".into())));
    assert!(matches!(error, TerminalFailure::Panicked(_)));
    assert!(std::error::Error::source(&error).is_some());
    assert!(!format!("{error:?} {error}").contains("sensitive cause"));
    Ok(())
}

#[test]
fn unexpected_matching_settlement_permits_cleanup_without_qualifying_success() -> Result<()> {
    let mut case = evidence::Case::new(options::CaseKind::ForeignGeneration, run()?);
    let process = ProcessKey { owner: case.run.owner, id: ProcessId::new(1)? };
    case.process = Some(process);
    assert!(
        case.observe_registry(RegistryObservation::FamilySettled { process, shutdown: None })
            .is_err()
    );
    assert!(!case.settled);
    assert!(!case.complete());
    assert!(!case.can_retire(false, true));
    assert!(!case.can_retire(true, false));
    assert!(case.can_retire(true, true));
    case.retired = true;
    assert!(!case.complete());
    assert!(!case.can_retire(true, true));
    Ok(())
}

#[test]
fn foreign_settlement_never_authorizes_current_cleanup_retirement() -> Result<()> {
    let mut case = evidence::Case::new(options::CaseKind::Valid, run()?);
    let process = ProcessKey { owner: case.run.owner, id: ProcessId::new(1)? };
    case.process = Some(process);
    assert!(
        case.observe_registry(RegistryObservation::FamilySettled {
            process: ProcessKey { id: ProcessId::new(2)?, ..process },
            shutdown: None,
        })
        .is_err()
    );
    assert!(!case.can_retire(true, true));
    assert!(!case.settled);
    Ok(())
}
