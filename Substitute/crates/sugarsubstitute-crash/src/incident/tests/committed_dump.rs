// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize committed summary matching and future-fact rejection without native authority.
use super::*;
use crate::incident::committed_dump::validation::validate_summary;
fn stored(ack: Option<i64>) -> Result<StoredIncidentSummary, Box<dyn Error>> {
    let original = seed()?;
    let dump = DumpId::new([7; 16])?;
    let attachment = AttachmentFacts::new(AttachmentKind::Minidump, 16, 16, 0, [9; 32])?;
    let inventory = AttachmentInventory::new(&[attachment])?;
    let summary = IncidentSummary::new(
        original,
        IncidentRevision::new(3)?,
        SummaryFacts::new(
            RetentionFacts::new(
                IncidentUtc::from_unix_seconds(101)?,
                ack.map(IncidentUtc::from_unix_seconds).transpose()?,
            )?,
            ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 },
            FailureEvidence::Unexplained,
            CaptureEvidence::Finished(dump),
            PayloadDisposition::Retained(inventory),
            ReportDisposition::new(0, None)?,
            SymbolOutcome::NotRequested,
        ),
    )?;
    Ok(StoredIncidentSummary::new(summary, Some(IncidentRevision::new(2)?))?)
}
#[test]
fn immutable_payload_generation_survives_later_summary_revision() -> Result<(), Box<dyn Error>> {
    let stored = stored(None)?;
    let key = stored.summary().seed().key();
    assert!(
        validate_summary(
            &stored,
            key,
            IncidentRevision::new(3)?,
            DumpId::new([7; 16])?,
            IncidentUtc::from_unix_seconds(200)?
        )
        .is_ok()
    );
    assert_eq!(stored.payload_revision(), Some(IncidentRevision::new(2)?));
    assert!(
        validate_summary(
            &stored,
            key,
            IncidentRevision::new(2)?,
            DumpId::new([7; 16])?,
            IncidentUtc::from_unix_seconds(200)?
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn historical_key_dump_and_expected_revision_are_all_exact() -> Result<(), Box<dyn Error>> {
    let stored = stored(None)?;
    let summary = stored.summary();
    let now = IncidentUtc::from_unix_seconds(200)?;
    let foreign = IncidentKey::new(IncidentSubject::Supervisor(owner(3)?));
    assert!(
        validate_summary(&stored, foreign, summary.revision(), DumpId::new([7; 16])?, now).is_err()
    );
    assert!(
        validate_summary(
            &stored,
            summary.seed().key(),
            summary.revision(),
            DumpId::new([8; 16])?,
            now
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn every_occurrence_and_acknowledgement_must_not_be_in_the_future() -> Result<(), Box<dyn Error>> {
    let stored = stored(Some(150))?;
    let summary = stored.summary();
    let key = summary.seed().key();
    let dump = DumpId::new([7; 16])?;
    for now in [99, 100, 149] {
        assert!(
            validate_summary(
                &stored,
                key,
                summary.revision(),
                dump,
                IncidentUtc::from_unix_seconds(now)?
            )
            .is_err()
        );
    }
    assert!(
        validate_summary(
            &stored,
            key,
            summary.revision(),
            dump,
            IncidentUtc::from_unix_seconds(150)?
        )
        .is_ok()
    );
    Ok(())
}
