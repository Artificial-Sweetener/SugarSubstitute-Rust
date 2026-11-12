// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Measure maximal closed summary members and actual bounded recovery behavior.
use super::support::*;

fn maximal_summary(instance: u64) -> Result<IncidentSummary, Box<dyn std::error::Error>> {
    let run = run(instance)?;
    let build = BuildIdentity::new(&"b".repeat(BuildIdentity::MAX_BYTES))?;
    let seed = IncidentSeed::new(
        IncidentKey::new(IncidentSubject::Desktop(run)),
        IncidentRevision::new(1)?,
        build.clone(),
        IncidentUtc::from_unix_seconds(100)?,
        IncidentStage::DesktopInitialization,
    )?;
    let dump = DumpId::new([3; 16])?;
    let release = ReleaseSymbols::new(
        build,
        FileLabel::new(&"m".repeat(FileLabel::MAX_BYTES))?,
        SymbolArchitecture::X86_64,
        PdbIdentity::new([4; 16], 1)?,
        [5; 32],
        [6; 32],
    );
    let frame = FailingFrame::new(
        dump,
        release,
        4096,
        0,
        SymbolName::new(&"s".repeat(SymbolName::MAX_BYTES))?,
        Some(SourceLine::new(FileLabel::new(&"f".repeat(FileLabel::MAX_BYTES))?, 1)?),
    );
    let mut attachments = Vec::new();
    for kind in [
        AttachmentKind::Startup,
        AttachmentKind::Fault,
        AttachmentKind::Runtime,
        AttachmentKind::Log,
        AttachmentKind::SymbolizedFault,
        AttachmentKind::Minidump,
    ] {
        attachments.push(AttachmentFacts::new(kind, 1, 1, 0, [7; 32])?);
    }
    Ok(IncidentSummary::new(
        seed,
        IncidentRevision::new(2)?,
        SummaryFacts::new(
            RetentionFacts::new(IncidentUtc::from_unix_seconds(101)?, None)?,
            ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 },
            FailureEvidence::Unexplained,
            CaptureEvidence::Finished(dump),
            PayloadDisposition::Retained(AttachmentInventory::new(&attachments)?),
            ReportDisposition::new(0, None)?,
            SymbolOutcome::Resolved(frame),
        ),
    )?)
}

#[test]
fn maximal_closed_summary_and_transition_fit_the_source_owned_bound() -> TestResult {
    let summary = maximal_summary(1)?;
    let SymbolOutcome::Resolved(frame) = summary.facts().symbols() else {
        return Err("Missing maximal frame".into());
    };
    let text = summary.seed().build().as_str().len()
        + frame.release().build().as_str().len()
        + frame.release().module().as_str().len()
        + frame.symbol().as_str().len()
        + frame.source().ok_or("Missing source line")?.file().as_str().len();
    assert_eq!(text, 896);
    assert_eq!(text + MAX_ATTACHMENTS * size_of::<Option<AttachmentFacts>>(), 1408);
    assert!(
        IncidentCoordinator::required_storage_bytes() > 128 * size_of::<IncidentSummary>() as u64
    );
    assert!(IncidentCoordinator::required_storage_bytes() + 32_768 < 2 * 1024 * 1024);
    let mut ready = ready(1)?;
    let previous = ready.summary.clone();
    present(&mut ready)?;
    assert_eq!(
        ready.coord.snapshot(previous.seed().key()).ok_or("Missing incident")?.revision,
        previous.revision().next()?
    );
    assert!(ready.coord.records.len() == MAX_INCIDENT_RECORDS);
    Ok(())
}

#[test]
fn full_maximal_recovery_rejects_the_next_incident_without_growth() -> TestResult {
    let mut coordinator = IncidentCoordinator::new(owner(3)?);
    let effects = coordinator.begin_recovery(context(1, 101)?.budget())?;
    let scope = effects
        .iter()
        .find_map(|effect| match effect {
            IncidentEffect::Recover { scope } => Some(*scope),
            _ => None,
        })
        .ok_or("Recovery scope absent")?;
    drop(effects);
    for instance in 1..=MAX_INCIDENT_RECORDS as u64 {
        let summary = maximal_summary(instance)?;
        coordinator.observe(IncidentServiceObservation::Recovered {
            scope,
            original_operation: Some(LogicalStoreOperationKey::new(
                owner(2)?,
                IncidentOperationId::new(instance)?,
            )),
            summary,
        })?;
    }
    assert_eq!(coordinator.records.iter().flatten().count(), MAX_INCIDENT_RECORDS);
    let result = coordinator.observe(IncidentServiceObservation::Recovered {
        scope,
        original_operation: Some(LogicalStoreOperationKey::new(
            owner(2)?,
            IncidentOperationId::new(129)?,
        )),
        summary: maximal_summary(129)?,
    });
    assert_eq!(result.err(), Some(IncidentDecisionError::Saturated));
    assert_eq!(coordinator.records.len(), MAX_INCIDENT_RECORDS);
    assert_eq!(coordinator.records.iter().flatten().count(), MAX_INCIDENT_RECORDS);
    assert!(coordinator.snapshot(IncidentKey::new(IncidentSubject::Desktop(run(128)?))).is_some());
    Ok(())
}
