// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject changed complete facts, payload generations and ledgers in an actual prepared view.
use super::*;
/// Exercise the snapshot produced by genuine8MiB load and actual320KiB presentation.
pub(super) fn check(
    view: &PreparedReportView,
    current: &StoredIncidentSummary,
    evictions: SummaryEvictions,
) -> TestResult {
    assert_eq!(view.metadata_claimed_bytes(), VIEW_METADATA_BYTES);
    assert!(size_of::<PreparedReportView>() < 1024);
    assert!(view.matches_current(current, evictions));
    let summary = current.summary();
    let changed_payload = StoredIncidentSummary::new(summary.clone(), Some(summary.revision()))?;
    assert_ne!(changed_payload.payload_revision(), current.payload_revision());
    assert!(!view.matches_current(&changed_payload, evictions));
    let changed_ledger = evictions.record(SummaryEviction::new(
        summary.seed().key(),
        IncidentUtc::from_unix_seconds(120)?,
        SummaryEvictionKind::Pending,
    ))?;
    assert!(!view.matches_current(current, changed_ledger));
    let facts = summary.facts();
    let changed_facts = StoredIncidentSummary::new(
        IncidentSummary::new(
            summary.seed().clone(),
            summary.revision(),
            SummaryFacts::new(
                facts.retention(),
                facts.exit(),
                facts.failure(),
                facts.capture(),
                facts.payload().clone(),
                ReportDisposition::new(8, Some(ReportAttempt::Presented))?,
                facts.symbols().clone(),
            ),
        )?,
        current.payload_revision(),
    )?;
    assert_eq!(changed_facts.summary().seed().key(), summary.seed().key());
    assert_eq!(changed_facts.summary().revision(), summary.revision());
    assert!(!view.matches_current(&changed_facts, evictions));
    assert!(view.matches_current(current, evictions));
    Ok(())
}
