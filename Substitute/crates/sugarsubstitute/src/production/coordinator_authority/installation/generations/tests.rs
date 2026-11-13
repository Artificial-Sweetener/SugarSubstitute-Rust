// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Old version cleanup never turns accepted health into a failure and always drains its worker.
use super::*;

fn issued(job: OwnedJob<(), io::Error>) -> Generations {
    let mut generations = Generations::new();
    generations.issued = true;
    generations.job = Some(job);
    generations
}

#[test]
fn a_finished_turn_releases_its_reservation_and_is_never_repeated() -> Result<(), Box<dyn Error>> {
    super::super::test_completion::completed(Ok(()), false, |job, batch, scope| {
        let mut generations = issued(job);
        assert!(generations.pending() && !generations.unissued());
        generations.consume(&batch)?;
        scope.require_released(JobId::new(2)?)?;
        assert!(!generations.pending());
        assert!(!generations.unissued());
        Ok(())
    })
}

#[test]
fn a_failed_turn_keeps_its_cause_and_leaves_the_application_open() -> Result<(), Box<dyn Error>> {
    super::super::test_completion::completed(
        Err(io::Error::other("version folder in use")),
        false,
        |job, batch, _scope| {
            let mut generations = issued(job);
            generations.consume(&batch)?;
            assert!(!generations.pending());
            let cause = generations
                .completion
                .as_ref()
                .and_then(|value| value.failure_source())
                .ok_or("Original cleanup error was lost")?;
            assert_eq!(cause.to_string(), "version folder in use");
            Ok(())
        },
    )
}

#[test]
fn a_turn_past_its_deadline_drains_without_failing_the_application() -> Result<(), Box<dyn Error>> {
    super::super::test_completion::completed(Ok(()), true, |job, batch, scope| {
        let mut generations = issued(job);
        generations.consume(&batch)?;
        assert!(!generations.pending());
        scope.require_released(JobId::new(2)?)?;
        assert!(
            generations
                .completion
                .as_ref()
                .is_some_and(|value| value.kind() == TerminalKind::TimedOut)
        );
        Ok(())
    })
}
