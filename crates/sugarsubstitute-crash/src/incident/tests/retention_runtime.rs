// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise semantic decisions through actual storage lifecycle and external boundary fakes.
use super::{
    publication_fixture::{admitted, setup},
    retention_fixture::*,
    *,
};
use sugarsubstitute_supervision::diagnostic_storage::*;

#[test]
fn payload_discard_actual_source_commits_summary_before_generation_reclamation() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let mut prepared = prepare(&fixture, &mut repository, &context, false)?;
        let original_file = fixture
            .snapshot()
            .active_identity(DiagnosticObject::Summary(prepared.original.summary().seed().key()))
            .ok_or("original Summary absent")?;
        let mut stage = repository.storage.seal_payload_discard(
            &mut prepared.preparation,
            &prepared.journal,
            &IncidentPayloadDiscardValidator,
            &repository.operation,
            &context,
        )?;
        assert!(fixture.snapshot().active_identity(prepared.payload).is_some());
        let mut permit = repository.storage.activate_payload_discard(
            &mut prepared.preparation,
            &mut stage,
            &repository.operation,
            &context,
        )?;
        assert!(fixture.snapshot().active_identity(prepared.payload).is_some());
        assert_eq!(
            fixture.snapshot().active_identity(DiagnosticObject::Summary(
                prepared.original.summary().seed().key()
            )),
            Some(original_file)
        );
        assert!(
            repository
                .record_attempt(
                    super::reporting_fixture::binding(prepared.original.summary())?,
                    ReportAttempt::Presented,
                    &context
                )
                .is_err()
        );
        let completion = repository.storage.complete_payload_discard(
            &mut permit,
            &repository.operation,
            &context,
        )?;
        assert_eq!(completion.reclaimed_bytes(), 29);
        assert_eq!(completion.reclaimed_payload_groups(), 1);
        assert_eq!(
            completion.summary().object(),
            DiagnosticObject::Summary(prepared.candidate.summary().seed().key())
        );
        assert!(fixture.snapshot().active_identity(prepared.payload).is_none());
        repository.storage.finish_payload_discard(
            &mut permit,
            &completion,
            &repository.operation,
            &context,
        )?;
        let loaded = repository.load(prepared.candidate.summary().seed().key(), &context)?;
        assert_eq!(loaded.summary(), prepared.candidate.summary());
        assert_eq!(
            loaded.summary().facts().reporting(),
            prepared.original.summary().facts().reporting()
        );
        assert_eq!(
            loaded.summary().facts().capture(),
            prepared.original.summary().facts().capture()
        );
        assert_eq!(
            loaded.summary().facts().retention(),
            prepared.original.summary().facts().retention()
        );
        assert!(loaded.payload_revision().is_none());
        repository.retire(&context)?;
        Ok(())
    })
}
#[test]
fn payload_discard_rejects_actual_candidate_with_changed_reporting_before_journal_activation()
-> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let mut prepared = prepare(&fixture, &mut repository, &context, true)?;
        let result = repository.storage.seal_payload_discard(
            &mut prepared.preparation,
            &prepared.journal,
            &IncidentPayloadDiscardValidator,
            &repository.operation,
            &context,
        );
        assert!(result.is_err());
        assert!(fixture.snapshot().active_identity(prepared.payload).is_some());
        assert!(
            fixture
                .snapshot()
                .active_identity(DiagnosticObject::Journal(repository.operation()))
                .is_none()
        );
        assert_eq!(repository.snapshot().reads(), 1);
        assert!(prepared.original_pin.retire(&context).is_err());
        assert!(prepared.candidate_pin.retire(&context).is_err());
        repository.retire(&context)?;
        Ok(())
    })
}
#[test]
fn payload_discard_external_decoder_failure_is_redacted_and_retains_original_cause() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let mut prepared = prepare(&fixture, &mut repository, &context, false)?;
        let sentinel = "PRIVATE_PAYLOAD_DISCARD_FIELD_58b";
        let original = std::str::from_utf8(&prepared.journal)?;
        let invalid = original
            .replace("\"payload_revision\":2", &format!("\"payload_revision\":2,\"{sentinel}\":1"));
        assert_ne!(invalid, original);
        let error = repository
            .storage
            .seal_payload_discard(
                &mut prepared.preparation,
                invalid.as_bytes(),
                &IncidentPayloadDiscardValidator,
                &repository.operation,
                &context,
            )
            .err()
            .ok_or("unknown field accepted")?;
        assert!(!error.to_string().contains(sentinel));
        assert!(!format!("{error:?}").contains(sentinel));
        let mut current: Option<&(dyn Error + 'static)> = Some(&error);
        let mut original_decoder = false;
        while let Some(cause) = current {
            if let Some(decoder) = cause.downcast_ref::<serde_json::Error>() {
                original_decoder = true;
                assert!(decoder.to_string().contains("Unknown incident field"));
                assert!(!decoder.to_string().contains(sentinel));
                assert!(decoder.line() > 0 && decoder.column() > 0);
            }
            current = cause.source();
        }
        assert!(original_decoder);
        assert!(fixture.snapshot().active_identity(prepared.payload).is_some());
        repository.retire(&context)?;
        Ok(())
    })
}

#[test]
fn payload_discard_failed_original_close_preserves_exact_intent_until_checked_retry() -> TestResult
{
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let mut prepared = prepare(&fixture, &mut repository, &context, false)?;
        let mut stage = repository.storage.seal_payload_discard(
            &mut prepared.preparation,
            &prepared.journal,
            &IncidentPayloadDiscardValidator,
            &repository.operation,
            &context,
        )?;
        let mut permit = repository.storage.activate_payload_discard(
            &mut prepared.preparation,
            &mut stage,
            &repository.operation,
            &context,
        )?;
        fixture.set_failure(
            sugarsubstitute_supervision::diagnostic_storage::fixture::DiagnosticFixtureFailure::Close,
            true,
        );
        assert!(
            repository
                .storage
                .complete_payload_discard(&mut permit, &repository.operation, &context,)
                .is_err()
        );
        assert!(fixture.snapshot().active_identity(prepared.payload).is_some());
        assert!(
            fixture
                .snapshot()
                .active_identity(DiagnosticObject::Journal(repository.operation()))
                .is_some()
        );
        assert_eq!(repository.snapshot().reads(), 1);
        assert!(repository.snapshot().writer_held());
        fixture.set_failure(
            sugarsubstitute_supervision::diagnostic_storage::fixture::DiagnosticFixtureFailure::Close,
            false,
        );
        let completion = repository.storage.complete_payload_discard(
            &mut permit,
            &repository.operation,
            &context,
        )?;
        assert_eq!(completion.reclaimed_bytes(), 29);
        repository.storage.finish_payload_discard(
            &mut permit,
            &completion,
            &repository.operation,
            &context,
        )?;
        repository.retire(&context)?;
        Ok(())
    })
}
