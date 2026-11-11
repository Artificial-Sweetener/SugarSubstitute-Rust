// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise policy correlation while projecting inert external symbol and store facts.
use super::support::*;

fn captured() -> Result<Ready, Box<dyn std::error::Error>> {
    ready_with_evidence(
        1,
        CollectedIncidentEvidence::new(
            IncidentUtc::from_unix_seconds(101)?,
            CaptureEvidence::Finished(DumpId::new([7; 16])?),
            PayloadDisposition::Unavailable,
            SymbolOutcome::NotRequested,
        ),
    )
}
fn resolved(
    original: &IncidentSummary,
    dump: DumpId,
    build: BuildIdentity,
) -> Result<IncidentSummary, Box<dyn std::error::Error>> {
    let facts = original.facts();
    let frame = FailingFrame::new(
        dump,
        ReleaseSymbols::new(
            build,
            FileLabel::new("SugarSubstitute.exe")?,
            SymbolArchitecture::X86_64,
            PdbIdentity::new([2; 16], 1)?,
            [3; 32],
            [4; 32],
        ),
        0x1001,
        1,
        SymbolName::new("fault")?,
        None,
    );
    Ok(IncidentSummary::new(
        original.seed().clone(),
        original.revision().next()?,
        SummaryFacts::new(
            facts.retention(),
            facts.exit(),
            facts.failure(),
            facts.capture(),
            facts.payload().clone(),
            facts.reporting(),
            SymbolOutcome::Resolved(frame),
        ),
    )?)
}

#[test]
fn symbol_commit_refreshes_same_session_once_and_preserves_original_scope() -> TestResult {
    let mut ready = captured()?;
    let old = ready.binding;
    let request = context(3, 102)?;
    let scope = ready.coord.admit_report_symbols(old, request)?;
    assert_eq!(scope.original(), request.budget());
    assert_eq!(
        ready.coord.admit_report_view(old, context(4, 103)?),
        Err(IncidentDecisionError::WrongPhase)
    );
    let summary = resolved(&ready.summary, DumpId::new([7; 16])?, ready.seed.build().clone())?;
    let observation = IncidentServiceObservation::SymbolsCommitted {
        scope,
        binding: old,
        summary: summary.clone(),
    };
    let effects = ready.coord.observe(observation.clone())?;
    let current = match effects.iter().next() {
        Some(IncidentEffect::LaunchReporter { binding, summary: actual }) => {
            assert_eq!(actual, &summary);
            *binding
        }
        _ => return Err("No refreshed launch".into()),
    };
    assert_eq!(current.session(), old.session());
    assert_eq!(current.revision(), old.revision().next()?);
    assert!(ready.coord.observe(observation)?.is_empty());
    assert_eq!(
        ready.coord.admit_report_view(old, context(4, 103)?),
        Err(IncidentDecisionError::StaleOperation)
    );
    assert!(ready.coord.admit_report_view(current, context(4, 103)?).is_ok());
    Ok(())
}

#[test]
fn symbol_commit_rejects_foreign_origin_release_dump_and_modified_evidence() -> TestResult {
    let mut ready = captured()?;
    let scope = ready.coord.admit_report_symbols(ready.binding, context(3, 102)?)?;
    let good = resolved(&ready.summary, DumpId::new([7; 16])?, ready.seed.build().clone())?;
    let mut foreign = scope;
    foreign.original = context(4, 103)?.budget();
    assert_eq!(
        ready.coord.observe(IncidentServiceObservation::SymbolsCommitted {
            scope: foreign,
            binding: ready.binding,
            summary: good
        }),
        Err(IncidentDecisionError::StaleOperation)
    );
    for (dump, build) in [
        (DumpId::new([8; 16])?, ready.seed.build().clone()),
        (DumpId::new([7; 16])?, BuildIdentity::new("foreign")?),
    ] {
        assert!(resolved(&ready.summary, dump, build).is_err());
    }
    let altered =
        changed(&ready.summary, ReportDisposition::new(1, Some(ReportAttempt::Presented))?, None)?;
    assert_eq!(
        ready.coord.observe(IncidentServiceObservation::SymbolsCommitted {
            scope,
            binding: ready.binding,
            summary: altered
        }),
        Err(IncidentDecisionError::MismatchedCommit)
    );
    assert_eq!(
        ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?.current_operation,
        Some(scope)
    );
    Ok(())
}

#[test]
fn symbol_admission_rejects_missing_dump_stale_binding_and_presented_record() -> TestResult {
    let mut missing = ready(1)?;
    let operation = missing.coord.next_operation;
    assert_eq!(
        missing.coord.admit_report_symbols(missing.binding, context(3, 102)?),
        Err(IncidentDecisionError::WrongPhase)
    );
    assert_eq!(missing.coord.next_operation, operation);
    let mut ready = captured()?;
    let stale = ready.binding;
    present(&mut ready)?;
    assert_eq!(
        ready.coord.admit_report_symbols(stale, context(4, 103)?),
        Err(IncidentDecisionError::StaleOperation)
    );
    assert_eq!(
        ready.coord.admit_report_symbols(ready.binding, context(4, 103)?),
        Err(IncidentDecisionError::WrongPhase)
    );
    Ok(())
}

#[test]
fn symbols_not_written_restores_same_summary_session_and_exact_operation_once() -> TestResult {
    let mut ready = captured()?;
    let original = ready.summary.clone();
    let binding = ready.binding;
    let scope = ready.coord.admit_report_symbols(binding, context(3, 102)?)?;
    let observation = IncidentServiceObservation::SymbolsNotWritten { scope, binding };
    let effects = ready.coord.observe(observation.clone())?;
    match effects.iter().next() {
        Some(IncidentEffect::LaunchReporter { binding: actual, summary }) => {
            assert_eq!(*actual, binding);
            assert_eq!(*summary, original);
        }
        _ => return Err("Original report was not restored".into()),
    }
    assert!(ready.coord.admit_report_view(binding, context(4, 103)?).is_ok());
    assert_eq!(ready.coord.observe(observation), Err(IncidentDecisionError::StaleOperation));
    Ok(())
}

#[test]
fn symbols_not_written_rejects_foreign_scope_binding_and_other_pending_work() -> TestResult {
    let mut ready = captured()?;
    let scope = ready.coord.admit_report_symbols(ready.binding, context(3, 102)?)?;
    let mut foreign = scope;
    foreign.original = context(4, 103)?.budget();
    assert_eq!(
        ready.coord.observe(IncidentServiceObservation::SymbolsNotWritten {
            scope: foreign,
            binding: ready.binding,
        }),
        Err(IncidentDecisionError::StaleOperation)
    );
    let wrong = ReportBinding::new(
        ready.binding.incident(),
        ready.binding.revision().next()?,
        ready.binding.session(),
    );
    assert_eq!(
        ready
            .coord
            .observe(IncidentServiceObservation::SymbolsNotWritten { scope, binding: wrong }),
        Err(IncidentDecisionError::MismatchedCommit)
    );
    assert_eq!(
        ready.coord.snapshot(ready.seed.key()).ok_or("No snapshot")?.current_operation,
        Some(scope)
    );
    let mut other = captured()?;
    let pending = other.coord.admit_report_view(other.binding, context(3, 102)?)?;
    assert_eq!(
        other.coord.observe(IncidentServiceObservation::SymbolsNotWritten {
            scope: pending,
            binding: other.binding,
        }),
        Err(IncidentDecisionError::StaleOperation)
    );
    Ok(())
}
