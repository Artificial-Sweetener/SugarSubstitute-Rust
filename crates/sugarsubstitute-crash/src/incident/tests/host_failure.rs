// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove an actual committed host failure survives reload and reaches copied report text.
use super::publication_fixture::{admitted_bytes, setup};
use super::*;
use crate::report_view::ReportView;
use sugarsubstitute_supervision::diagnostic_storage::DIAGNOSTIC_STORAGE_BYTES;

#[test]
fn committed_host_failure_is_reported_without_a_native_dump()
-> Result<(), Box<dyn Error + Send + Sync>> {
    admitted_bytes(DIAGNOSTIC_STORAGE_BYTES + ReportView::MAX_BYTES as u64 + 4096, |context| {
        let (_fixture, mut repository) = setup(&context)?;
        let original =
            super::summary().map_err(|cause| std::io::Error::other(cause.to_string()))?;
        let host = HostFailureFact::new(
            "main_frontend_effect",
            "None",
            HostFailureCategory::Io,
            Some("Io"),
            None,
            1,
            1,
        )?;
        let chunk = RedactedTextChunk::from_host_failure(&host);
        let preflight = repository.preflight_publication(
            original.seed().clone(),
            ExpectedSummary::Absent,
            IncidentPublicationDemand::new(chunk.as_bytes().len() as u64, 1, 1, 131072)?,
            IncidentUtc::from_unix_seconds(111)?,
            &context,
        )?;
        let mut ticket =
            repository.begin_publish(preflight, IncidentOperationId::new(1)?, &context)?;
        let attachment =
            repository.append_text(&mut ticket, AttachmentKind::Fault, chunk, &context)?;
        let facts = original.facts();
        let summary = IncidentSummary::new(
            original.seed().clone(),
            original.revision(),
            SummaryFacts::new(
                facts.retention(),
                facts.exit(),
                facts.failure(),
                facts.capture(),
                PayloadDisposition::Retained(AttachmentInventory::new(&[attachment])?),
                facts.reporting(),
                facts.symbols().clone(),
            ),
        )?;
        repository.commit(&mut ticket, summary, &context)?;
        let current = repository.load(original.seed().key(), &context)?;
        assert_eq!(repository.read_host_failure(&current, &context)?, Some(host.clone()));
        let restored =
            repository.read_host_failure(&current, &context)?.ok_or("Host failure missing")?;
        let tail = RedactedTextChunk::from_host_failure(&restored);
        let binding = ReportBinding::new(
            current.summary().seed().key(),
            current.summary().revision(),
            ReportSessionId::new(1)?,
        );
        for locale in sugarsubstitute_contracts::localization::LOCALES {
            let view = ReportView::format(
                current.summary(),
                binding,
                current.evictions(),
                std::slice::from_ref(&tail),
                locale.id,
                context.memory(),
                false,
            )?;
            assert!(view.text().contains("operation=main_frontend_effect"));
            assert!(view.text().contains("category=Io detail=Io"));
            let labels = locale.report;
            assert!(view.text().contains(labels.host_failed));
            assert!(!view.text().contains(labels.unexplained));
            assert!(view.text().contains(&format!("{}: {}", labels.symbols, labels.unavailable)));
        }
        repository.retire(&context)?;
        Ok(())
    })
}
