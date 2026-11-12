// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify stored host facts cross the real load, cleanup and separately funded view stages.
use super::*;
use sha2::{Digest, Sha256};

#[test]
fn copied_report_includes_the_stored_host_failure_after_file_cleanup() -> TestResult {
    let (owner, wake, scope) = execution()?;
    let client = owner.client();
    let mut load =
        client.submit_blocking(&scope, spec(1, DIAGNOSTIC_STORAGE_BYTES)?, |context| {
            let fixture = DiagnosticStorageFixture::new()?;
            let mut operation = prepared(&context, &fixture)?;
            let original = original()?;
            let host = HostFailureFact::new(
                "main_frontend_effect",
                "None",
                HostFailureCategory::Io,
                Some("Io"),
                None,
                1,
                1,
            )?;
            let bytes = RedactedTextChunk::from_host_failure(&host);
            let attachment = AttachmentFacts::new(
                AttachmentKind::Fault,
                bytes.as_bytes().len() as u64,
                bytes.original_bytes(),
                bytes.omitted_bytes(),
                Sha256::digest(bytes.as_bytes()).into(),
            )?;
            let facts = original.facts();
            let summary = IncidentSummary::new(
                original.seed().clone(),
                original.revision(),
                SummaryFacts::new(
                    facts.retention(),
                    facts.exit(),
                    FailureEvidence::Unexplained,
                    CaptureEvidence::Unavailable(CaptureAbsence::Missing),
                    PayloadDisposition::Retained(AttachmentInventory::new(&[attachment])?),
                    facts.reporting(),
                    SymbolOutcome::NotRequested,
                ),
            )?;
            fixture.put(
                DiagnosticObject::Summary(summary.seed().key()),
                &encode_summary(&StoredIncidentSummary::new(
                    summary.clone(),
                    Some(summary.revision()),
                )?)?,
            )?;
            fixture.put(
                DiagnosticObject::Payload(
                    summary.seed().key(),
                    summary.revision(),
                    AttachmentKind::Fault,
                ),
                bytes.as_bytes(),
            )?;
            operation.load_current(binding(&summary)?, "en", false, &context)?;
            assert!(operation.retire(&context));
            Ok::<_, TestError>(operation)
        })?;
    let mut operation = success(completed(&client, &wake, &mut load)?)?;
    let mut view =
        client.submit_blocking(&scope, spec(2, REPORT_VIEW_JOB_BYTES)?, move |context| {
            operation.prepare_view(&context)?;
            let content = operation.take_view().ok_or("Actual report view missing")?;
            let text = content.view().text();
            assert!(text.contains("The application reported a failure"));
            assert!(text.contains("operation=main_frontend_effect"));
            assert!(text.contains("category=Io detail=Io"));
            assert!(!text.contains("Unexplained termination"));
            assert!(text.contains("Failing frame: Unavailable"));
            Ok::<_, TestError>(())
        })?;
    success(completed(&client, &wake, &mut view)?)?;
    drop(load);
    drop(view);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
