// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual bounded formatting, identity admission, Unicode and retained credit.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::{execution::MemoryRelease, lifecycle::*};
struct Release(Arc<AtomicU64>);
impl MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::SeqCst);
    }
}
fn lease(bytes: u64) -> (MemoryLease, Arc<AtomicU64>) {
    let released = Arc::new(AtomicU64::new(0));
    (MemoryLease::new(bytes, Box::new(Release(released.clone()))), released)
}
fn summary() -> Result<IncidentSummary, Box<dyn Error>> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let key = IncidentKey::new(IncidentSubject::Desktop(RunKey {
        owner,
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    }));
    let seed = IncidentSeed::new(
        key,
        IncidentRevision::new(1)?,
        BuildIdentity::new("release_1")?,
        IncidentUtc::from_unix_seconds(100)?,
        IncidentStage::Startup,
    )?;
    Ok(IncidentSummary::new(
        seed,
        IncidentRevision::new(2)?,
        SummaryFacts::new(
            RetentionFacts::new(IncidentUtc::from_unix_seconds(101)?, None)?,
            ExitAttribution::Desktop { kind: DesktopExitKind::Unexpected, status: 0 },
            FailureEvidence::Unexplained,
            CaptureEvidence::Unavailable(CaptureAbsence::Missing),
            PayloadDisposition::Unavailable,
            ReportDisposition::new(0, None)?,
            SymbolOutcome::NotRequested,
        ),
    )?)
}
fn binding(summary: &IncidentSummary) -> Result<ReportBinding, IncidentError> {
    Ok(ReportBinding::new(summary.seed().key(), summary.revision(), ReportSessionId::new(1)?))
}
fn ledger(summary: &IncidentSummary) -> Result<SummaryEvictions, IncidentError> {
    SummaryEvictions::new(summary.seed().key().subject().coordinator().namespace, 0, 0, 0, None)
}
fn facts_summary(
    failure: FailureEvidence,
    capture: CaptureEvidence,
    payload: PayloadDisposition,
    symbols: SymbolOutcome,
) -> Result<IncidentSummary, Box<dyn Error>> {
    let s = summary()?;
    Ok(IncidentSummary::new(
        s.seed().clone(),
        s.revision(),
        SummaryFacts::new(
            s.facts().retention(),
            s.facts().exit(),
            failure,
            capture,
            payload,
            s.facts().reporting(),
            symbols,
        ),
    )?)
}
#[test]
fn report_preserves_each_capture_absence_and_symbol_failure_in_every_locale()
-> Result<(), Box<dyn Error>> {
    let (lease, _) = lease(1_000_000);
    let captures = [
        CaptureAbsence::HandlerUnavailable,
        CaptureAbsence::HandlerLost,
        CaptureAbsence::Missing,
        CaptureAbsence::Busy,
        CaptureAbsence::DeadlineElapsed,
        CaptureAbsence::WrongIdentity,
        CaptureAbsence::Corrupt,
        CaptureAbsence::Oversized,
    ];
    let symbols = [
        SymbolFailure::MissingDump,
        SymbolFailure::CorruptDump,
        SymbolFailure::MissingSymbols,
        SymbolFailure::WrongRelease,
        SymbolFailure::UnknownModule,
        SymbolFailure::UnresolvedAddress,
        SymbolFailure::DeadlineElapsed,
        SymbolFailure::HelperFailed,
    ];
    for locale in ["en", "zh-Hans", "ja", "ko", "es"] {
        for (index, (capture, symbol)) in captures.into_iter().zip(symbols).enumerate() {
            let s = facts_summary(
                FailureEvidence::Unexplained,
                CaptureEvidence::Unavailable(capture),
                PayloadDisposition::Quarantined,
                SymbolOutcome::Unresolved(UnresolvedSymbols::new(symbol, None)?),
            )?;
            let view =
                ReportView::format(&s, binding(&s)?, ledger(&s)?, &[], locale, &lease, false)?;
            assert!(view.text().contains(messages(locale).report.capture_absence[index]));
            assert!(view.text().contains(messages(locale).report.symbol_failure[index]));
            assert!(view.text().contains(messages(locale).report.quarantined));
        }
    }
    Ok(())
}
#[test]
fn report_startup_failure_retains_actual_category_without_unexpected_or_native_inference()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::startup::StartupFailureCode;
    let (lease, _) = lease(1_000_000);
    for code in [
        StartupFailureCode::ConnectionPreparation,
        StartupFailureCode::ConnectionUnavailable,
        StartupFailureCode::ProcessExited,
        StartupFailureCode::TimedOut,
        StartupFailureCode::Presentation,
    ] {
        let s = facts_summary(
            FailureEvidence::Startup(code),
            CaptureEvidence::Unavailable(CaptureAbsence::HandlerUnavailable),
            PayloadDisposition::Unavailable,
            SymbolOutcome::NotRequested,
        )?;
        let view = ReportView::format(&s, binding(&s)?, ledger(&s)?, &[], "en", &lease, false)?;
        assert!(view.text().contains(messages("en").report.startup_failure[startup_reason(code)]));
        assert!(!view.text().contains("stopped unexpectedly"));
        assert!(!view.text().contains(messages("en").report.native_fault));
    }
    Ok(())
}
#[test]
fn unavailable_capture_does_not_infer_clean_exit_or_native_fault() -> Result<(), Box<dyn Error>> {
    let s = summary()?;
    let (lease, _) = lease(1_000_000);
    let view = ReportView::format(&s, binding(&s)?, ledger(&s)?, &[], "en", &lease, false)?;
    assert!(view.text().contains("Unexplained termination"));
    assert!(view.text().contains("Exit status: 0"));
    assert!(!view.text().contains("Native exception"));
    assert!(!view.text().contains(['\0', '\r']));
    assert!(view.text().ends_with('\n'));
    Ok(())
}
#[test]
fn exact_namespace_revision_and_inventory_reject_before_credit() -> Result<(), Box<dyn Error>> {
    let s = summary()?;
    let (lease, _) = lease(ReportView::MAX_BYTES as u64 + ReportView::TRANSFER_HEADER_BYTES);
    let wrong =
        ReportBinding::new(s.seed().key(), IncidentRevision::new(3)?, ReportSessionId::new(1)?);
    assert!(matches!(
        ReportView::format(&s, wrong, ledger(&s)?, &[], "en", &lease, false),
        Err(ReportViewError::Identity)
    ));
    let wrong_ledger = SummaryEvictions::new(NamespaceId::new([2; 32]), 0, 0, 0, None)?;
    assert!(matches!(
        ReportView::format(&s, binding(&s)?, wrong_ledger, &[], "en", &lease, false),
        Err(ReportViewError::Identity)
    ));
    let tails: Vec<_> = (0..5)
        .map(|_| {
            RedactedTextChunk::from_facts(&[crate::incident::RedactedDiagnosticFact::Stage(
                IncidentStage::Startup,
            )])
        })
        .collect::<Result<_, _>>()?;
    assert!(matches!(
        ReportView::format(&s, binding(&s)?, ledger(&s)?, &tails, "en", &lease, false),
        Err(ReportViewError::Tails)
    ));
    let claim = lease.claim(lease.bytes())?;
    drop(claim);
    Ok(())
}
#[test]
fn transferred_text_keeps_one_credit_until_actual_allocation_drop() -> Result<(), Box<dyn Error>> {
    let s = summary()?;
    let bytes = ReportView::MAX_BYTES as u64 + ReportView::TRANSFER_HEADER_BYTES;
    let (lease, released) = lease(bytes);
    let view = ReportView::format(&s, binding(&s)?, ledger(&s)?, &[], "en", &lease, false)?;
    assert!(lease.claim(1).is_err());
    let (_, text, claim) = view.into_parts();
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    assert!(text.capacity() <= ReportView::MAX_BYTES);
    drop(text);
    drop(claim);
    assert_eq!(released.load(Ordering::SeqCst), bytes);
    Ok(())
}
#[test]
fn every_registered_report_locale_formats_utf8_with_canonical_fallback()
-> Result<(), Box<dyn Error>> {
    let s = summary()?;
    let (lease, _) = lease(1_000_000);
    for locale in ["en", "zh-Hans", "ja", "ko", "es", "unknown"] {
        let view = ReportView::format(&s, binding(&s)?, ledger(&s)?, &[], locale, &lease, false)?;
        assert!(view.text().starts_with(messages(locale).report.title));
        assert!(view.text().contains(messages(locale).report.unexplained));
    }
    Ok(())
}
#[test]
fn bounded_writer_preserves_unicode_and_explicit_omission_inside_ceiling()
-> Result<(), Box<dyn Error>> {
    let mut writer = Writer::new(24, "[omitted]")?;
    writer.bytes("中文中文中文中文中文".as_bytes());
    writer.line("ignored after omission");
    let text = writer.finish();
    assert!(text.len() <= 24);
    assert!(text.capacity() <= 24);
    assert!(text.ends_with("[omitted]\n"));
    assert!(std::str::from_utf8(text.as_bytes()).is_ok());
    assert!(Writer::new(3, "marker").is_err());
    Ok(())
}

mod degraded;

mod capture_only;
