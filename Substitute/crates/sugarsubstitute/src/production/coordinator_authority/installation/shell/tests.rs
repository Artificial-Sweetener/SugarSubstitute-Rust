// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Optional shell errors and actual worker expiry never turn accepted health into startup failure.
use super::*;

#[test]
fn successful_shell_publication_releases_scratch_before_shutdown() -> Result<(), Box<dyn Error>> {
    super::super::test_completion::completed(Ok(()), false, |job, batch, scope| {
        let mut shell = Shell::new();
        shell.issued = true;
        shell.job = Some(job);
        assert_eq!(scope.reserved_bytes(), 4096);
        shell.consume(&batch)?;
        scope.require_released(JobId::new(2)?)?;
        assert!(!shell.pending());
        assert!(!shell.unissued());
        Ok(())
    })
}

#[test]
fn shell_failure_is_retained_and_does_not_close_a_healthy_application() -> Result<(), Box<dyn Error>>
{
    super::super::test_completion::completed(
        Err(io::Error::other("preserved user shortcut")),
        false,
        |job, batch, scope| {
            let mut shell = Shell::new();
            shell.issued = true;
            shell.job = Some(job);
            assert!(shell.pending());
            shell.consume(&batch)?;
            assert!(!shell.pending());
            assert!(!shell.unissued());
            let cause = shell
                .completion
                .as_ref()
                .and_then(|value| value.failure_source())
                .ok_or("Original shell error was lost")?;
            assert_eq!(scope.reserved_bytes(), 4096);
            assert_eq!(cause.to_string(), "preserved user shortcut");
            Ok(())
        },
    )
}

#[test]
fn shell_deadline_is_observed_without_invalidating_accepted_health() -> Result<(), Box<dyn Error>> {
    super::super::test_completion::completed(Ok(()), true, |job, batch, scope| {
        let mut shell = Shell::new();
        shell.issued = true;
        shell.job = Some(job);
        shell.consume(&batch)?;
        assert!(!shell.pending());
        scope.require_released(JobId::new(2)?)?;
        assert!(!shell.unissued());
        assert!(
            shell.completion.as_ref().is_some_and(|value| value.kind() == TerminalKind::TimedOut)
        );
        Ok(())
    })
}

#[test]
fn shell_ignores_unrelated_notices_and_rejects_a_forged_terminal_kind() -> Result<(), Box<dyn Error>>
{
    super::super::test_completion::completed(Ok(()), false, |job, batch, scope| {
        let mut shell = Shell::new();
        shell.issued = true;
        shell.job = Some(job);
        let mut foreign = CompletionBatch {
            notifications: batch.notifications.clone(),
            remaining: batch.remaining,
        };
        for notice in &mut foreign.notifications {
            notice.job_id = JobId::new(2)?;
        }
        shell.consume(&foreign)?;
        assert!(shell.pending());
        let mut forged = batch;
        for notice in &mut forged.notifications {
            notice.kind = TerminalKind::Failure;
        }
        assert_eq!(shell.consume(&forged), Err(AuthorityIssue::Identity));
        assert!(!shell.pending());
        scope.require_released(JobId::new(2)?)?;
        assert!(
            shell.completion.as_ref().is_some_and(|value| value.kind() == TerminalKind::Success)
        );
        Ok(())
    })
}
