// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual execution terminals while faking only the storage operation result.
use super::*;

#[test]
fn successful_publication_releases_scratch_while_retaining_its_observed_decision()
-> Result<(), Box<dyn Error>> {
    super::super::test_completion::completed(Ok(()), false, |job, batch, scope| {
        let mut publication = Publication::new();
        publication.job = Some(job);
        assert_eq!(scope.reserved_bytes(), 4096);
        publication.consume(&batch)?;
        assert!(publication.succeeded());
        assert!(!publication.unissued());
        scope.require_released(JobId::new(2)?)?;
        publication.consume(&batch)?;
        assert!(publication.succeeded());
        Ok(())
    })
}
/// Feed observed native worker terminals to the real health owner.
fn completed(
    result: io::Result<()>,
    check: impl FnOnce(&mut Publication, CompletionBatch) -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    super::super::test_completion::completed(result, false, |job, batch, _scope| {
        let mut health = Publication::new();
        assert!(health.unissued());
        health.job = Some(job);
        assert!(health.pending());
        assert!(!health.unissued());
        check(&mut health, batch)?;
        assert!(!health.pending());
        assert!(!health.unissued());
        Ok(())
    })
}

#[test]
fn health_completion_ignores_unrelated_notices_and_retires_only_its_actual_job()
-> Result<(), Box<dyn Error>> {
    completed(Ok(()), |health, batch| {
        let mut foreign = CompletionBatch {
            notifications: batch.notifications.clone(),
            remaining: batch.remaining,
        };
        for notice in &mut foreign.notifications {
            notice.job_id = JobId::new(2)?;
        }
        health.consume(&foreign)?;
        assert!(health.pending());
        health.consume(&batch)?;
        health.consume(&batch)?;
        assert!(health.failure_source().is_none());
        assert!(health.succeeded());
        assert!(matches!(
            health.completion.as_ref().map(|value| value.kind()),
            Some(TerminalKind::Success)
        ));
        Ok(())
    })
}

#[test]
fn health_completion_preserves_storage_failure_even_when_notice_kind_is_forged()
-> Result<(), Box<dyn Error>> {
    completed(Err(io::Error::other("original publication failure")), |health, mut batch| {
        for notice in &mut batch.notifications {
            notice.kind = TerminalKind::Success;
        }
        assert_eq!(health.consume(&batch), Err(AuthorityIssue::Identity));
        assert_eq!(
            health.failure_source().ok_or("Original failure missing")?.to_string(),
            "original publication failure"
        );
        Ok(())
    })
}

#[test]
fn finalization_must_consume_the_original_success_before_authority_can_settle()
-> Result<(), Box<dyn Error>> {
    super::super::test_completion::completed(Ok(()), false, |job, batch, _scope| {
        let mut authority =
            CoordinatorAuthority::new(EntrySetupBudget::new(Moment::from_nanos(0))?, None)?;
        authority.request_close();
        authority.installation.settlement.job = Some(job);
        assert!(!authority.settled());
        authority.installation.consume(&CompletionBatch { notifications: vec![], remaining: 0 })?;
        assert!(!authority.settled());
        authority.installation.consume(&batch)?;
        assert!(authority.settled());
        authority.installation.consume(&batch)?;
        assert!(authority.settled());
        Ok(())
    })
}

#[test]
fn failed_or_discarded_finalization_does_not_report_settled_or_allow_second_publication()
-> Result<(), Box<dyn Error>> {
    for expire in [false, true] {
        let result = if expire { Ok(()) } else { Err(io::Error::other("publication failed")) };
        super::super::test_completion::completed(result, expire, |job, batch, _scope| {
            let mut authority =
                CoordinatorAuthority::new(EntrySetupBudget::new(Moment::from_nanos(0))?, None)?;
            authority.request_close();
            authority.installation.settlement.job = Some(job);
            assert_eq!(authority.installation.consume(&batch), Err(AuthorityIssue::Stopped));
            assert!(!authority.pending());
            assert!(!authority.settled());
            assert!(!authority.installation.settlement.unissued());
            if !expire {
                assert_eq!(
                    authority.installation.failure_source().ok_or("Cause lost")?.to_string(),
                    "publication failed"
                );
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn mismatched_notice_cannot_turn_an_actual_success_into_confirmed_publication()
-> Result<(), Box<dyn Error>> {
    completed(Ok(()), |publication, mut batch| {
        for notice in &mut batch.notifications {
            notice.kind = TerminalKind::TimedOut;
        }
        assert_eq!(publication.consume(&batch), Err(AuthorityIssue::Identity));
        assert!(!publication.succeeded());
        assert!(matches!(
            publication.completion.as_ref().map(|value| value.kind()),
            Some(TerminalKind::Success)
        ));
        Ok(())
    })
}

#[test]
fn helper_finalization_waits_for_observed_health_writer_even_when_result_is_discarded()
-> Result<(), Box<dyn Error>> {
    for expire in [false, true] {
        super::super::test_completion::completed(Ok(()), expire, |job, batch, _scope| {
            let mut authority =
                CoordinatorAuthority::new(EntrySetupBudget::new(Moment::from_nanos(0))?, None)?;
            authority.request_close();
            authority.installation.health.job = Some(job);
            assert!(!authority.installation_writers_retired());
            authority
                .installation
                .consume(&CompletionBatch { notifications: vec![], remaining: 0 })?;
            assert!(!authority.installation_writers_retired());
            // A discarded health result leaves acceptance unpublished without failing the app.
            assert_eq!(authority.installation.consume(&batch), Ok(()));
            assert!(authority.installation_writers_retired());
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn a_failed_health_publication_keeps_the_app_open_and_the_trial_without_a_verdict()
-> Result<(), Box<dyn Error>> {
    for expire in [false, true] {
        let result = if expire { Ok(()) } else { Err(io::Error::other("selection stayed locked")) };
        super::super::test_completion::completed(result, expire, |job, batch, _scope| {
            let mut authority =
                CoordinatorAuthority::new(EntrySetupBudget::new(Moment::from_nanos(0))?, None)?;
            authority.installation.observe_readiness();
            authority.installation.health.job = Some(job);
            authority.installation.consume(&batch)?;
            assert!(!authority.closing);
            assert!(authority.failure.is_none());
            assert!(!authority.installation.pending());
            assert!(!authority.installation.health.succeeded());
            assert_eq!(
                authority.installation.verdict(),
                TrialVerdict::NoVerdict(
                    sugarsubstitute_contracts::installation::Undecided::AcceptanceUnpublished,
                )
            );
            if !expire {
                assert_eq!(
                    authority.installation.failure_source().ok_or("Cause lost")?.to_string(),
                    "selection stayed locked"
                );
            }
            Ok(())
        })?;
    }
    Ok(())
}
