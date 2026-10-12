// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Test actual byte ceilings, unique inventory and independent capture/fault facts.
use super::*;

#[test]
fn attachment_cap_includes_marker_and_preserves_omission_counts() -> Result<(), IncidentError> {
    let text = AttachmentFacts::new(
        AttachmentKind::Startup,
        MAX_TEXT_ATTACHMENT_BYTES,
        MAX_TEXT_ATTACHMENT_BYTES + 17,
        40,
        [1; 32],
    )?;
    assert_eq!(text.kind(), AttachmentKind::Startup);
    assert_eq!(text.retained_bytes(), 262_144);
    assert_eq!(text.original_bytes(), 262_161);
    assert_eq!(text.omitted_bytes(), 40);
    // The retained text includes a 23-byte marker; it is not copied source content.
    assert_eq!(text.original_bytes() - text.omitted_bytes(), text.retained_bytes() - 23);
    assert_eq!(text.sha256(), &[1; 32]);
    for kind in [
        AttachmentKind::Startup,
        AttachmentKind::Fault,
        AttachmentKind::Runtime,
        AttachmentKind::Log,
        AttachmentKind::SymbolizedFault,
    ] {
        assert_eq!(
            AttachmentFacts::new(kind, MAX_TEXT_ATTACHMENT_BYTES + 1, u64::MAX, 0, [0; 32]),
            Err(IncidentError::InvalidEvidence)
        );
    }
    assert_eq!(
        AttachmentFacts::new(AttachmentKind::Log, 10, 9, 10, [0; 32]),
        Err(IncidentError::InvalidEvidence)
    );
    assert_eq!(
        AttachmentFacts::new(AttachmentKind::Log, 0, 0, 0, [0; 32]),
        Err(IncidentError::InvalidEvidence)
    );
    Ok(())
}

#[test]
fn native_dump_is_never_represented_as_truncated_or_text_sanitized() -> Result<(), IncidentError> {
    let dump = AttachmentFacts::new(
        AttachmentKind::Minidump,
        MAX_PAYLOAD_BYTES,
        MAX_PAYLOAD_BYTES,
        0,
        [2; 32],
    )?;
    assert_eq!(dump.omitted_bytes(), 0);
    assert_eq!(
        AttachmentFacts::new(AttachmentKind::Minidump, 1, 2, 1, [0; 32]),
        Err(IncidentError::InvalidEvidence)
    );
    assert_eq!(
        AttachmentFacts::new(AttachmentKind::Minidump, MAX_PAYLOAD_BYTES + 1, u64::MAX, 0, [0; 32]),
        Err(IncidentError::InvalidEvidence)
    );
    Ok(())
}

#[test]
fn discarded_payload_preserves_oversized_source_fact_without_admitting_it()
-> Result<(), IncidentError> {
    assert_eq!(
        DiscardedPayload::new(PayloadDiscard::StoragePressure, 0),
        Err(IncidentError::InvalidEvidence)
    );
    let discarded = DiscardedPayload::new(PayloadDiscard::Oversized, u64::MAX)?;
    assert_eq!(discarded.reason(), PayloadDiscard::Oversized);
    assert_eq!(discarded.source_bytes(), u64::MAX);
    assert_eq!(
        AttachmentFacts::new(
            AttachmentKind::Minidump,
            discarded.source_bytes(),
            discarded.source_bytes(),
            0,
            [0; 32],
        ),
        Err(IncidentError::InvalidEvidence)
    );
    Ok(())
}

#[test]
fn inventory_rejects_duplicate_kind_count_and_aggregate_pressure() -> Result<(), IncidentError> {
    let text = AttachmentFacts::new(AttachmentKind::Log, 1, 1, 0, [1; 32])?;
    assert_eq!(AttachmentInventory::new(&[]), Err(IncidentError::InvalidEvidence));
    assert_eq!(AttachmentInventory::new(&[text, text]), Err(IncidentError::InvalidEvidence));
    assert_eq!(
        AttachmentInventory::new(&[text; MAX_ATTACHMENTS + 1]),
        Err(IncidentError::InvalidEvidence)
    );
    let dump = AttachmentFacts::new(
        AttachmentKind::Minidump,
        MAX_PAYLOAD_BYTES,
        MAX_PAYLOAD_BYTES,
        0,
        [2; 32],
    )?;
    assert_eq!(AttachmentInventory::new(&[dump, text]), Err(IncidentError::InvalidEvidence));
    let inventory = AttachmentInventory::new(&[dump])?;
    assert_eq!(inventory.total_bytes(), 268_435_456);
    assert_eq!(inventory.entries().copied().collect::<Vec<_>>(), [dump]);
    let startup = AttachmentFacts::new(AttachmentKind::Startup, 2, 3, 1, [3; 32])?;
    let inventory = AttachmentInventory::new(&[startup, text])?;
    assert_eq!(inventory.total_bytes(), 3);
    assert_eq!(inventory.entries().copied().collect::<Vec<_>>(), [startup, text]);
    Ok(())
}

#[test]
fn missing_capture_does_not_change_supplied_fault_or_exit_attribution()
-> Result<(), Box<dyn std::error::Error>> {
    let key = desktop()?;
    for capture in [
        CaptureEvidence::Awaiting,
        CaptureEvidence::Unavailable(CaptureAbsence::Missing),
        CaptureEvidence::Unavailable(CaptureAbsence::HandlerLost),
    ] {
        let supplied = FailureEvidence::NativeFault { code: 0xc000_0005, address: 0x1010 };
        let summary = IncidentSummary::new(
            seed(key)?,
            IncidentRevision::new(1)?,
            facts(
                ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 },
                supplied,
                capture,
                SymbolOutcome::Unresolved(UnresolvedSymbols::new(
                    SymbolFailure::MissingDump,
                    None,
                )?),
            )?,
        )?;
        assert_eq!(summary.facts().failure(), supplied);
        assert_eq!(summary.facts().capture(), capture);
        assert_eq!(
            summary.facts().exit(),
            ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 }
        );
    }
    Ok(())
}
