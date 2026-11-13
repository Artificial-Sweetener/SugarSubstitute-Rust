// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real repository/execution with an external filesystem fixture; no native proof is fabricated.
use super::{
    publication_fixture::{admitted, setup},
    reporting_fixture::*,
    *,
};
use sugarsubstitute_supervision::diagnostic_storage::*;
#[test]
fn pending_scan_requires_actual_retirement_and_preserves_original_summary_bytes() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let summary = original()?;
        installed(&fixture, summary.clone(), None)?;
        let object = DiagnosticObject::Summary(summary.seed().key());
        let before = fixture.snapshot().active_identity(object).ok_or("Original absent")?;
        let changes = fixture.snapshot().mutation_count();
        let mut pending =
            repository.pending_incidents(IncidentUtc::from_unix_seconds(120)?, &context)?;
        assert!(pending.summaries().is_err());
        repository.retire_pending(&mut pending, &context)?;
        let mut retained = pending.summaries()?;
        let original = retained.next().ok_or("Pending summary absent")?;
        assert_eq!(original.summary(), &summary);
        assert_eq!(original.payload_revision(), None);
        assert!(retained.next().is_none());
        assert_eq!(fixture.snapshot().active_identity(object), Some(before));
        assert_eq!(fixture.snapshot().mutation_count(), changes);
        Ok(())
    })
}
#[test]
fn acknowledged_summary_is_not_reopened_and_corrupt_pending_bytes_fail_closed() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let summary = original()?;
        installed(&fixture, summary.clone(), None)?;
        repository.acknowledge(
            ReportCommand::new(binding(&summary)?, ReportAction::Dismiss),
            IncidentUtc::from_unix_seconds(110)?,
            &context,
        )?;
        let mut pending =
            repository.pending_incidents(IncidentUtc::from_unix_seconds(120)?, &context)?;
        repository.retire_pending(&mut pending, &context)?;
        assert_eq!(pending.summaries()?.count(), 0);
        Ok(())
    })?;
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let object = DiagnosticObject::Summary(original()?.seed().key());
        let before = fixture.put(object, b"corrupt but preserved pending bytes")?;
        let changes = fixture.snapshot().mutation_count();
        assert!(
            repository.pending_incidents(IncidentUtc::from_unix_seconds(120)?, &context).is_err()
        );
        assert_eq!(fixture.snapshot().active_identity(object), Some(before));
        assert_eq!(fixture.snapshot().mutation_count(), changes);
        repository.retire(&context)?;
        Ok(())
    })
}
