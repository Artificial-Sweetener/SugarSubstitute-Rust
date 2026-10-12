// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject malformed input and preserve exact wire/context fields.
use super::{super::*, support::*};
use sugarsubstitute_contracts::incident::*;
#[test]
fn recover_has_exact_schema1_prefix_and_original_counters() -> TestResult {
    let mut f = storage(1, DiagnosticCommand::Recover)?;
    f.context.incident = None;
    let bytes = encoded(f)?;
    assert_eq!(&bytes[..14], &[83, 83, 68, 73, 1, 2, 13, 0, 159, 0, 0, 0, 0, 0]);
    assert_eq!(decode(&bytes)?, f);
    Ok(())
}
#[test]
fn publication_preserves_demand_utc_full_historical_subject_and_borrow() -> TestResult {
    let demand = DiagnosticPublicationDemand::new(7, 1, 0, 131_072)?;
    let f = storage(
        1,
        DiagnosticCommand::Publish(DiagnosticPublication::Begin {
            expected: DiagnosticExpectedSummary::Exact {
                revision: IncidentRevision::new(5)?,
                sha256: [3; 32],
            },
            demand,
            seed: b"{}",
        }),
    )?;
    let bytes = encoded(f)?;
    let decoded = decode(&bytes)?;
    assert_eq!(decoded, f);
    assert_ne!(
        decoded.key.owner().epoch,
        decoded.context.incident.ok_or("subject")?.key.subject().coordinator().epoch
    );
    let DiagnosticCommand::Publish(DiagnosticPublication::Begin { seed, .. }) = decoded.command
    else {
        return Err("publication".into());
    };
    assert_eq!(seed.as_ptr(), bytes[bytes.len() - 2..].as_ptr());
    Ok(())
}
#[test]
fn malformed_schema_reserved_fields_and_trailing_bytes_reject() -> TestResult {
    let bytes = encoded(storage(1, DiagnosticCommand::Load)?)?;
    for (offset, value) in [(0, 0), (4, 2), (5, 99), (6, 128), (7, 1)] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        assert!(decode(&bad).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(decode(&trailing), Err(DiagnosticWireError::Shape));
    for length in 0..14 {
        assert!(decode(&bytes[..length]).is_err())
    }
    Ok(())
}
#[test]
fn wrong_current_owner_role_and_missing_utc_reject_before_output() -> TestResult {
    let mut f = storage(1, DiagnosticCommand::Load)?;
    let mut output = [0xa5; 1024];
    f.context.at = None;
    assert_eq!(encode(f, &mut output), Err(DiagnosticWireError::Shape));
    assert!(output.iter().all(|b| *b == 0xa5));
    f.context.at = Some(IncidentUtc::from_unix_seconds(20)?);
    f.key = key(DiagnosticRole::Reporter)?;
    assert_eq!(encode(f, &mut output), Err(DiagnosticWireError::Role));
    f.key = key(DiagnosticRole::Storage)?;
    f.context.operation =
        Some(LogicalStoreOperationKey::new(owner(2)?, IncidentOperationId::new(6)?));
    assert_eq!(encode(f, &mut output), Err(DiagnosticWireError::Identity));
    Ok(())
}
#[test]
fn impossible_demand_and_predecessor_slot_confusion_reject() -> TestResult {
    for args in [
        (1, 0, 1, 131_072),
        (0, 1, 1, 131_072),
        (1, 1, 2, 131_072),
        (1, 1, 1, 131_071),
        (u64::MAX, 1, 1, 131_072),
    ] {
        assert_eq!(
            DiagnosticPublicationDemand::new(args.0, args.1, args.2, args.3),
            Err(DiagnosticWireError::Capacity)
        )
    }
    let f = storage(
        1,
        DiagnosticCommand::Publish(DiagnosticPublication::Begin {
            expected: DiagnosticExpectedSummary::Absent,
            demand: DiagnosticPublicationDemand::new(0, 0, 0, 131_072)?,
            seed: b"{}",
        }),
    )?;
    assert_eq!(encoded(f), Err(DiagnosticWireError::Shape));
    Ok(())
}
#[test]
fn frame_header_chunk_and_output_capacity_are_literal() -> TestResult {
    let binding = binding()?;
    let bytes = vec![b'x'; MAX_CHUNK_BYTES];
    let chunk = DiagnosticChunk {
        kind: DiagnosticTransferKind::View,
        total: bytes.len() as u64,
        offset: 0,
        sha256: [1; 32],
        bytes: &bytes,
    };
    let f = reporter(1, DiagnosticCommand::ViewChunk { session: binding.session(), chunk }, true)?;
    let encoded = encoded(f)?;
    assert!(encoded.len() <= MAX_FRAME_BYTES);
    assert_eq!(decode(&encoded)?, f);
    let mut short = [7; 10];
    assert_eq!(encode(f, &mut short), Err(DiagnosticWireError::Capacity));
    assert_eq!(short, [7; 10]);
    let mut bad = encoded.clone();
    bad[8..10].copy_from_slice(&1025u16.to_le_bytes());
    assert_eq!(decode(&bad), Err(DiagnosticWireError::Capacity));
    assert_eq!(decode(&vec![0; MAX_FRAME_BYTES + 1]), Err(DiagnosticWireError::Capacity));
    Ok(())
}
#[test]
fn noncanonical_boolean_and_selection_nul_reject() -> TestResult {
    let f = storage(
        1,
        DiagnosticCommand::Acknowledge { session: binding()?.session(), restart: false },
    )?;
    let mut bytes = encoded(f)?;
    let last = bytes.len() - 1;
    bytes[last] = 2;
    assert_eq!(decode(&bytes), Err(DiagnosticWireError::Schema));
    let selection = DiagnosticSelection {
        id: DiagnosticRequestId::new(3)?,
        selection: ExportSelection::TextAndNativeDump,
        overwrite: true,
        destination: &[b'C', 0, 0, 0],
    };
    assert_eq!(
        encoded(storage(2, DiagnosticCommand::RetainExport(selection))?),
        Err(DiagnosticWireError::Shape)
    );
    Ok(())
}
#[test]
fn unknown_fields_and_passive_deadline_do_not_create_authority() -> TestResult {
    let mut f = reporter(1, DiagnosticCommand::Presented(binding()?.session()), false)?;
    f.context.deadline = Some(DiagnosticDeadline::new(1, 1)?);
    assert_eq!(encoded(f), Err(DiagnosticWireError::Shape));
    let mut bytes = encoded(storage(1, DiagnosticCommand::Load)?)?;
    let n = bytes.len();
    bytes.push(0);
    bytes[8..10].copy_from_slice(&u16::try_from(n + 1)?.to_le_bytes());
    assert_eq!(decode(&bytes), Err(DiagnosticWireError::Shape));
    Ok(())
}
#[test]
fn replacing_old_or_equal_revision_rejects() -> TestResult {
    let s = binding()?.session();
    let f = reporter(
        1,
        DiagnosticCommand::ReplaceView {
            session: s,
            old: IncidentRevision::new(5)?,
            total: 1,
            sha256: [1; 32],
        },
        true,
    )?;
    assert_eq!(encoded(f), Err(DiagnosticWireError::Shape));
    Ok(())
}
#[test]
fn closed_commands_roundtrip_without_paths_as_root_authority() -> TestResult {
    let session = binding()?.session();
    let digest = [1; 32];
    let dump = DumpId::new([1; 16])?;
    let selection = DiagnosticSelection {
        id: DiagnosticRequestId::new(1)?,
        selection: ExportSelection::TextOnly,
        overwrite: false,
        destination: &[b'C', 0, b':', 0],
    };
    let chunk = DiagnosticChunk {
        kind: DiagnosticTransferKind::Summary,
        total: 2,
        offset: 0,
        sha256: digest,
        bytes: b"{}",
    };
    let commands = [
        DiagnosticCommand::Seed(b"{}"),
        DiagnosticCommand::Publish(DiagnosticPublication::Chunk(chunk)),
        DiagnosticCommand::Publish(DiagnosticPublication::Finish { bytes: 2, sha256: digest }),
        DiagnosticCommand::Load,
        DiagnosticCommand::RetainExport(selection),
        DiagnosticCommand::ExportChunk(chunk),
        DiagnosticCommand::FinishExport(digest),
        DiagnosticCommand::Acknowledge { session, restart: true },
        DiagnosticCommand::RecordAttempt {
            session,
            attempt: ReportAttempt::Failed(ReportFailure::Load),
        },
        DiagnosticCommand::RetainDump(dump),
        DiagnosticCommand::DiscardCapture(dump),
    ];
    for command in commands {
        let f = storage(1, command)?;
        assert_eq!(decode(&encoded(f)?)?, f)
    }
    let mut f = storage(1, DiagnosticCommand::Symbolize { dump, release: digest })?;
    f.key = key(DiagnosticRole::Symbolize)?;
    assert_eq!(decode(&encoded(f)?)?, f);
    let mut f = storage(1, DiagnosticCommand::OpenProtectedRoot)?;
    f.context.incident = None;
    assert_eq!(decode(&encoded(f)?)?, f);
    for role in [DiagnosticRole::Storage, DiagnosticRole::Symbolize, DiagnosticRole::Reporter] {
        let f = DiagnosticFrame {
            key: key(role)?,
            request: DiagnosticRequestId::new(1)?,
            context: DiagnosticContext {
                deadline: Some(DiagnosticDeadline::new(1, 1)?),
                ..DiagnosticContext::default()
            },
            command: DiagnosticCommand::Retire,
        };
        assert_eq!(decode(&encoded(f)?)?, f)
    }
    Ok(())
}
#[test]
fn reporter_command_and_failure_categories_roundtrip() -> TestResult {
    let s = binding()?.session();
    let digest = [1; 32];
    let chunk = DiagnosticChunk {
        kind: DiagnosticTransferKind::View,
        total: 1,
        offset: 0,
        sha256: digest,
        bytes: b"x",
    };
    let ack = DurableAcknowledgement::new(
        subject()?.key,
        subject()?.revision,
        IncidentUtc::from_unix_seconds(20)?,
    );
    for command in [
        DiagnosticCommand::BeginView { session: s, total: 1, sha256: digest },
        DiagnosticCommand::ViewChunk { session: s, chunk },
        DiagnosticCommand::EndView { session: s, sha256: digest },
        DiagnosticCommand::ReplaceView {
            session: s,
            old: IncidentRevision::new(4)?,
            total: 1,
            sha256: digest,
        },
        DiagnosticCommand::CloseAfterDurableAck { session: s, acknowledgement: ack },
        DiagnosticCommand::CopyResult { session: s, result: DiagnosticActionResult::Success },
        DiagnosticCommand::ExportSelectionResult {
            session: s,
            result: DiagnosticActionResult::Failed(ReportFailure::Export),
        },
    ] {
        let f = reporter(1, command, true)?;
        assert_eq!(decode(&encoded(f)?)?, f)
    }
    for command in [
        DiagnosticCommand::Presented(s),
        DiagnosticCommand::ViewReplaced(s),
        DiagnosticCommand::SaveSelectionCancelled(s),
        DiagnosticCommand::NativeRetired(s),
    ] {
        let f = reporter(1, command, false)?;
        assert_eq!(decode(&encoded(f)?)?, f)
    }
    for action in [
        ReportAction::CopyText,
        ReportAction::Export(ExportSelection::TextOnly),
        ReportAction::Export(ExportSelection::TextAndNativeDump),
        ReportAction::Dismiss,
        ReportAction::DismissAndRequestRestart,
    ] {
        let f = reporter(1, DiagnosticCommand::Action { session: s, action }, false)?;
        assert_eq!(decode(&encoded(f)?)?, f)
    }
    for failure in [
        ReportFailure::Launch,
        ReportFailure::Load,
        ReportFailure::Presentation,
        ReportFailure::UnexpectedExit,
        ReportFailure::Acknowledgement,
        ReportFailure::Export,
        ReportFailure::Retirement,
    ] {
        let f = reporter(1, DiagnosticCommand::ReporterFailed { session: s, failure }, false)?;
        assert_eq!(decode(&encoded(f)?)?, f)
    }
    let selection = DiagnosticSelection {
        id: DiagnosticRequestId::new(2)?,
        selection: ExportSelection::TextAndNativeDump,
        overwrite: true,
        destination: &[b'C', 0],
    };
    let f = reporter(1, DiagnosticCommand::SaveSelectionReady { session: s, selection }, false)?;
    assert_eq!(decode(&encoded(f)?)?, f);
    Ok(())
}
