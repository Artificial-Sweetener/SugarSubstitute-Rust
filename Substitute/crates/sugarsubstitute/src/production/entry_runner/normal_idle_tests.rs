// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove pending actual runner obligations prevent any external admission fence effect.
use super::*;
use std::cell::Cell;
fn ready() -> Admission {
    Admission {
        requested: true,
        initial_completed: true,
        initial_retired: true,
        intake_ready: true,
        pending_activation: false,
        forwarding: false,
        closing: false,
        failure: false,
    }
}
#[test]
fn only_accepted_normal_shutdown_suppresses_a_nested_failure_dialog() -> Result<(), Box<dyn Error>>
{
    #[derive(Debug)]
    struct Wrapped(Box<dyn Error>);
    impl fmt::Display for Wrapped {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("Native lifetime failed")
        }
    }
    impl Error for Wrapped {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(self.0.as_ref())
        }
    }
    for normal in [false, true] {
        let path = PathBuf::from("unopened-relative-input");
        let command = ProductionCommand {
            launch_selection: None,
            installation: path.clone(),
            diagnostics: Default::default(),
            options: super::super::tests::unopened_paths()?.into(),
        };
        let mut lifetime = Box::new(EntryRunner::new(
            command,
            path,
            Box::new(super::super::tests::NoChild(None, false)),
            crate::diagnostics::DiagnosticHealth::fixture(),
        ));
        lifetime.normal_shutdown = normal;
        let failure = retain_failure("Original shutdown failure".into(), lifetime);
        assert_eq!(
            is_normal_shutdown_failure::<super::super::tests::NoChild>(failure.as_ref()),
            normal
        );
        let nested = Wrapped(failure);
        assert_eq!(is_normal_shutdown_failure::<super::super::tests::NoChild>(&nested), normal);
        assert!(format!("{:?}", FailureDisplay(&nested)).contains("Original shutdown failure"));
        let shown = Cell::new(false);
        let failure: Box<dyn Error> = Box::new(nested);
        let original = failure.as_ref() as *const dyn Error;
        let reported = crate::production::entry_failure::report_failure::<
            super::super::tests::NoChild,
            std::io::Error,
        >(failure, |_, _, _, _| {
            shown.set(true);
            Ok(())
        });
        assert_eq!(shown.get(), !normal);
        assert!(std::ptr::eq(original, reported.as_ref() as *const dyn Error));
    }
    Ok(())
}
#[test]
fn normal_idle_runner_retains_pending_projection_initial_terminal_forward_and_failure_before_fence()
-> Result<(), Box<dyn Error>> {
    for obstruction in 0..8 {
        let mut state = ready();
        match obstruction {
            0 => state.requested = false,
            1 => state.initial_completed = false,
            2 => state.initial_retired = false,
            3 => state.intake_ready = false,
            4 => state.pending_activation = true,
            5 => state.forwarding = true,
            6 => state.closing = true,
            _ => state.failure = true,
        }
        let effects = Cell::new(0);
        assert!(!state.try_fence(|| {
            effects.set(effects.get() + 1);
            Ok(true)
        })?);
        assert_eq!(effects.get(), 0);
    }
    Ok(())
}
#[test]
fn normal_idle_runner_requires_actual_atomic_empty_intake_and_preserves_first_fence_error()
-> Result<(), Box<dyn Error>> {
    let effects = Cell::new(0);
    assert!(!ready().try_fence(|| {
        effects.set(effects.get() + 1);
        Ok(false)
    })?);
    assert_eq!(effects.get(), 1);
    assert!(ready().try_fence(|| {
        effects.set(effects.get() + 1);
        Ok(true)
    })?);
    assert_eq!(effects.get(), 2);
    #[derive(Debug)]
    struct OriginalFenceError;
    impl std::fmt::Display for OriginalFenceError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("Original admission fence error")
        }
    }
    impl Error for OriginalFenceError {}
    let cause = ready()
        .try_fence(|| Err(Box::new(OriginalFenceError)))
        .err()
        .ok_or("Fence failure lost")?;
    assert!(cause.downcast_ref::<OriginalFenceError>().is_some());
    Ok(())
}

fn runner() -> Result<EntryRunner<super::super::tests::NoChild>, Box<dyn Error>> {
    let path = PathBuf::from("unopened-relative-input");
    let command = ProductionCommand {
        launch_selection: None,
        installation: path.clone(),
        diagnostics: Default::default(),
        options: super::super::tests::unopened_paths()?.into(),
    };
    let mut runner = EntryRunner::new(
        command,
        path,
        Box::new(super::super::tests::NoChild(None, true)),
        crate::diagnostics::DiagnosticHealth::fixture(),
    );
    super::super::tests::freeze_time(&mut runner);
    runner.authority = Some(CoordinatorAuthority::new(
        sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(
            sugarsubstitute_contracts::execution::Moment::from_nanos(0),
        )?,
        None,
    )?);
    runner.launch_update =
        crate::updates::launch::LaunchUpdate::new(Some(crate::updates::UpdatePolicy::BetaEager));
    Ok(runner)
}

/// The admission fence for the runner as it stands; the initial client's transport completion
/// is a real job completion, so its observed success is supplied once the test reaches it.
fn admission(
    runner: &EntryRunner<super::super::tests::NoChild>,
    initial_retired: bool,
) -> Admission {
    let mut admission = runner.normal_admission();
    admission.initial_retired |= initial_retired;
    admission
}

/// The coordinator's restart decision, as its update turn makes it.
fn decide_restart(
    runner: &mut EntryRunner<super::super::tests::NoChild>,
) -> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::update::attempt::AttemptOutcome;
    runner.observe_window();
    runner.launch_update.finish(&AttemptOutcome::Prepared { version: "2.0.0".into() });
    runner.announce_restart()
}

/// The window opening, as the parent lifecycle reports MainRevealed to the authority.
fn reveal(runner: &mut EntryRunner<super::super::tests::NoChild>) -> Result<(), Box<dyn Error>> {
    runner.authority.as_mut().ok_or("authority absent")?.observe_window_opened();
    Ok(())
}

/// A beta-eager check that prepares an update before the window opened says which version it
/// restarts into at once. A live desktop child is never closed by the coordinator: a window
/// that was never revealed closes itself to restart, through its own normal close.
#[test]
fn a_restart_before_the_window_leaves_a_live_desktop_to_close_itself() -> Result<(), Box<dyn Error>>
{
    use sugarsubstitute_contracts::update::status::LaunchVersion;
    let mut runner = runner()?;
    runner.initial_completed = true;
    runner.intake_ready = true;
    decide_restart(&mut runner)?;
    let status = runner.update_status.snapshot().1;
    assert!(status.restarting(), "the window is told to stay hidden");
    assert_eq!(status.applying(), Some("2.0.0"));
    assert_eq!(runner.launch_version(), Some(LaunchVersion::Restarting("2.0.0".into())));
    let fences = Cell::new(0);
    runner.close_after_fence(admission(&runner, true), |_| {
        fences.set(fences.get() + 1);
        Ok(true)
    })?;
    assert_eq!(fences.get(), 0, "the coordinator never cuts off a live window");
    assert!(!runner.closing);
    assert!(runner.launch_update.restart().is_some(), "the restart still stands");
    Ok(())
}

/// Without a desktop child there is no window to keep hidden: the restart waits only for the
/// close fence, never closes past an in-flight entry request, then closes normally and
/// relaunches into the update. The fence is supplied here: the real one is the entry intake's
/// `try_close_when_idle`, which needs a live entry server.
#[test]
fn a_restart_before_any_window_waits_for_the_close_fence_then_closes_normally_and_relaunches()
-> Result<(), Box<dyn Error>> {
    let mut runner = runner()?;
    runner.child.1 = false;
    decide_restart(&mut runner)?;
    let fences = Cell::new(0);
    let fence = |idle: bool| {
        fences.set(fences.get() + 1);
        Ok::<_, Box<dyn Error>>(idle)
    };
    let pending = admission(&runner, false);
    assert!(pending.requested, "the restart is a requested close");
    runner.close_after_fence(pending, |_| fence(true))?;
    assert_eq!(fences.get(), 0, "intake not ready: the fence is not even asked");
    assert!(!runner.closing && !runner.normal_shutdown);

    runner.initial_completed = true;
    runner.intake_ready = true;
    runner.close_after_fence(admission(&runner, true), |_| fence(false))?;
    assert_eq!(fences.get(), 1);
    assert!(!runner.closing, "an in-flight entry request holds the close");

    runner.close_after_fence(admission(&runner, true), |_| fence(true))?;
    assert_eq!(fences.get(), 2);
    assert!(runner.closing && runner.normal_shutdown);
    assert!(runner.relaunch_wanted());
    Ok(())
}

/// A window revealed before it processed the restart stays open: the coordinator owes no close
/// with or without a desktop child, the next update step withdraws the restart and tells the
/// window, and the update waits for the next launch.
#[test]
fn a_window_revealed_before_processing_the_restart_is_never_closed() -> Result<(), Box<dyn Error>> {
    let mut runner = runner()?;
    runner.initial_completed = true;
    runner.intake_ready = true;
    decide_restart(&mut runner)?;
    reveal(&mut runner)?;
    runner.child.1 = false;
    let fences = Cell::new(0);
    runner.close_after_fence(admission(&runner, true), |_| {
        fences.set(fences.get() + 1);
        Ok(true)
    })?;
    assert_eq!(fences.get(), 0, "a seen window owes no close");
    assert!(!runner.closing);

    runner.observe_window();
    runner.publish_update_status()?;
    let status = runner.update_status.snapshot().1;
    assert!(!status.restarting(), "the window may open");
    assert_eq!(status.applying(), None);
    assert!(runner.launch_update.restart().is_none());
    runner.normal_shutdown = true;
    assert!(!runner.relaunch_wanted(), "the update waits for the next launch");
    Ok(())
}
