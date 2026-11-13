// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinator release uses the real executor and original terminal, independently of its join.
use super::*;
use sugarsubstitute_contracts::execution::ScopeSpec;

#[test]
fn release_requires_closed_authority_and_observed_admission() -> Result<(), Box<dyn Error>> {
    super::super::test_completion::completed(
        Ok(None::<Arc<RunningApplication>>),
        false,
        |job, batch, capacity| {
            let mut authority =
                CoordinatorAuthority::new(EntrySetupBudget::new(Moment::from_nanos(0))?, None)?;
            let jobs = JobSequence::new();
            assert_eq!(
                authority.retire_installation(capacity.client(), capacity.scope(), &jobs),
                Err(AuthorityIssue::Pending)
            );
            authority.request_close();
            authority.installation.job = Some(job);
            assert_eq!(
                authority.retire_installation(capacity.client(), capacity.scope(), &jobs),
                Err(AuthorityIssue::Pending)
            );
            authority.installation.consume(&batch)?;
            assert!(authority.settled());
            Ok(())
        },
    )
}

#[test]
fn coordinator_releases_admission_custody_before_executor_join() -> Result<(), Box<dyn Error>> {
    super::super::test_completion::completed(
        Ok(None::<Arc<RunningApplication>>),
        false,
        |job, batch, capacity| {
            let mut authority =
                CoordinatorAuthority::new(EntrySetupBudget::new(Moment::from_nanos(0))?, None)?;
            authority.installation.job = Some(job);
            authority.installation.consume(&batch)?;
            assert!(authority.installation.ready());
            let jobs = JobSequence::new();
            jobs.allocate()?;
            let cleanup = capacity
                .client()
                .create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
            authority.submit_close(
                capacity.client(),
                &cleanup,
                &jobs,
                Moment::from_nanos(1_000_000_000),
            )?;
            assert!(!authority.retire_installation(capacity.client(), &cleanup, &jobs)?);
            assert!(authority.installation.completion.is_none());
            assert!(authority.installation.release.job.is_some());
            capacity.observe(|| {
                authority
                    .retire_installation(capacity.client(), &cleanup, &jobs)
                    .map_err(Into::into)
            })?;
            assert!(authority.installation.release.job.is_none());
            assert!(authority.retire_installation(capacity.client(), &cleanup, &jobs)?);
            capacity.require_released(jobs.allocate()?)?;
            Ok(())
        },
    )
}

#[test]
fn rejected_release_preserves_original_custody_and_reports_its_cause() -> Result<(), Box<dyn Error>>
{
    super::super::test_completion::completed(
        Ok(None::<Arc<RunningApplication>>),
        false,
        |job, batch, capacity| {
            let mut authority =
                CoordinatorAuthority::new(EntrySetupBudget::new(Moment::from_nanos(0))?, None)?;
            authority.installation.job = Some(job);
            authority.installation.consume(&batch)?;
            let jobs = JobSequence::new();
            jobs.allocate()?;
            authority.submit_close(
                capacity.client(),
                capacity.scope(),
                &jobs,
                Moment::from_nanos(1_000_000_000),
            )?;
            let native = capacity
                .client()
                .create_native_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
            assert_eq!(
                authority.retire_installation(capacity.client(), &native, &jobs),
                Err(AuthorityIssue::Adapter)
            );
            assert!(authority.installation.ready());
            assert!(authority.installation.release.job.is_none());
            assert!(authority.cleanup_failure().is_some());
            assert_eq!(
                authority.retire_installation(capacity.client(), capacity.scope(), &jobs),
                Err(AuthorityIssue::Adapter)
            );
            assert!(authority.installation.ready());
            Ok(())
        },
    )
}

#[test]
fn expired_cleanup_never_moves_custody_or_renews_the_cutoff() -> Result<(), Box<dyn Error>> {
    super::super::test_completion::completed(
        Ok(None::<Arc<RunningApplication>>),
        false,
        |job, batch, capacity| {
            let mut authority =
                CoordinatorAuthority::new(EntrySetupBudget::new(Moment::from_nanos(0))?, None)?;
            authority.installation.job = Some(job);
            authority.installation.consume(&batch)?;
            let jobs = JobSequence::new();
            authority.submit_close(
                capacity.client(),
                capacity.scope(),
                &jobs,
                Moment::from_nanos(0),
            )?;
            assert_eq!(
                authority.retire_installation(capacity.client(), capacity.scope(), &jobs),
                Err(AuthorityIssue::Closed)
            );
            assert!(authority.installation.ready());
            authority.submit_close(
                capacity.client(),
                capacity.scope(),
                &jobs,
                Moment::from_nanos(1_000_000_000),
            )?;
            assert_eq!(authority.close_until, Some(Moment::from_nanos(0)));
            assert_eq!(
                authority.retire_installation(capacity.client(), capacity.scope(), &jobs),
                Err(AuthorityIssue::Closed)
            );
            assert!(authority.installation.ready());
            assert!(authority.installation.release.job.is_none());
            Ok(())
        },
    )
}

#[test]
fn release_preserves_failed_admission_diagnostics() -> Result<(), Box<dyn Error>> {
    super::super::test_completion::completed(
        Err::<Option<Arc<RunningApplication>>, _>(io::Error::other(
            "original installation failure",
        )),
        false,
        |job, batch, capacity| {
            let mut authority =
                CoordinatorAuthority::new(EntrySetupBudget::new(Moment::from_nanos(0))?, None)?;
            authority.installation.job = Some(job);
            assert_eq!(authority.installation.consume(&batch), Err(AuthorityIssue::Stopped));
            authority.request_close();
            assert!(authority.retire_installation(
                capacity.client(),
                capacity.scope(),
                &JobSequence::new()
            )?);
            assert_eq!(
                authority
                    .installation
                    .failure_source()
                    .ok_or("Original failure missing")?
                    .to_string(),
                "original installation failure"
            );
            assert!(authority.installation.release.job.is_none());
            Ok(())
        },
    )
}

#[test]
fn stopped_release_never_reports_success_or_loses_its_original_cause() -> Result<(), Box<dyn Error>>
{
    for expire in [false, true] {
        let result = if expire { Ok(()) } else { Err(io::Error::other("release failed")) };
        super::super::test_completion::completed(result, expire, |job, _batch, _capacity| {
            let mut release = Release::new();
            release.job = Some(job);
            assert_eq!(release.observe(), Err(AuthorityIssue::Stopped));
            assert_eq!(release.observe(), Err(AuthorityIssue::Stopped));
            assert!(release.job.is_none());
            if !expire {
                assert_eq!(
                    release.failure_source().ok_or("Release cause missing")?.to_string(),
                    "release failed"
                );
            }
            Ok(())
        })?;
    }
    Ok(())
}
