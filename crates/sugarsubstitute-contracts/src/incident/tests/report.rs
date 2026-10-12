// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep report intent, observed presentation, durable acknowledgement and restart apart.
use super::*;

#[test]
fn report_actions_never_acknowledge_or_change_a_summary() -> Result<(), Box<dyn std::error::Error>>
{
    let key = desktop()?;
    let summary = IncidentSummary::new(
        seed(key)?,
        IncidentRevision::new(1)?,
        facts(
            ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 },
            FailureEvidence::Unexplained,
            CaptureEvidence::Awaiting,
            SymbolOutcome::NotRequested,
        )?,
    )?;
    let original = summary.clone();
    let binding = ReportBinding::new(key, summary.revision(), ReportSessionId::new(1)?);
    for action in [
        ReportAction::CopyText,
        ReportAction::Export(ExportSelection::TextOnly),
        ReportAction::Export(ExportSelection::TextAndNativeDump),
        ReportAction::Dismiss,
        ReportAction::DismissAndRequestRestart,
    ] {
        let command = ReportCommand::new(binding, action);
        assert_eq!(command.action(), action);
        assert_eq!(command.binding(), binding);
        assert_eq!(summary, original);
        assert_eq!(summary.facts().retention().acknowledged(), None);
    }
    Ok(())
}

#[test]
fn report_binding_preserves_expected_revision_and_session() -> Result<(), Box<dyn std::error::Error>>
{
    assert_eq!(ReportSessionId::new(0), Err(IncidentError::InvalidIdentity));
    let key = desktop()?;
    let binding = ReportBinding::new(key, IncidentRevision::new(1)?, ReportSessionId::new(2)?);
    assert_eq!(binding.incident(), key);
    assert_eq!(binding.revision().get(), 1);
    assert_eq!(binding.session().get(), 2);
    assert_ne!(binding, ReportBinding::new(key, IncidentRevision::new(2)?, binding.session()));
    assert_ne!(binding, ReportBinding::new(key, binding.revision(), ReportSessionId::new(3)?));
    assert_ne!(
        binding,
        ReportBinding::new(
            IncidentKey::new(IncidentSubject::Supervisor(owner()?)),
            binding.revision(),
            binding.session(),
        )
    );
    Ok(())
}

#[test]
fn presentation_failure_and_durable_acknowledgement_are_distinct_observations()
-> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        ReportDisposition::new(0, Some(ReportAttempt::Presented)),
        Err(IncidentError::MismatchedFacts)
    );
    assert_eq!(ReportDisposition::new(1, None), Err(IncidentError::MismatchedFacts));
    let failed =
        ReportDisposition::new(1, Some(ReportAttempt::Failed(ReportFailure::UnexpectedExit)))?;
    assert_eq!(failed.attempts(), 1);
    assert_eq!(failed.last(), Some(ReportAttempt::Failed(ReportFailure::UnexpectedExit)));
    let key = desktop()?;
    let binding = ReportBinding::new(key, IncidentRevision::new(1)?, ReportSessionId::new(1)?);
    let presented = ReportObservation::Presented(binding);
    let failed = ReportObservation::Failed { binding, cause: ReportFailure::Acknowledgement };
    let ack = DurableAcknowledgement::new(
        key,
        IncidentRevision::new(2)?,
        IncidentUtc::from_unix_seconds(201)?,
    );
    assert_eq!(ack.incident(), key);
    assert_eq!(ack.revision().get(), 2);
    assert_eq!(ack.at().unix_seconds(), 201);
    assert_ne!(presented, ReportObservation::Acknowledged(ack));
    assert_ne!(failed, ReportObservation::Acknowledged(ack));
    assert_eq!(ExportSelection::default(), ExportSelection::TextOnly);
    Ok(())
}
