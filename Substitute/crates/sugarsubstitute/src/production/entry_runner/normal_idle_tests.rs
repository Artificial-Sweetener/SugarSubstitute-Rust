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
            Box::new(super::super::tests::NoChild(None)),
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
        Box::new(super::super::tests::NoChild(None)),
        crate::diagnostics::DiagnosticHealth::fixture(),
    );
    super::super::tests::freeze_time(&mut runner);
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

/// A beta-eager check that prepares an update before intake is ready owes a restart: it says
/// which version it applies at once, waits for the close fence, never closes past an in-flight
/// entry request, and then closes normally and relaunches into the update.
#[test]
fn a_restart_before_the_window_waits_for_the_close_fence_then_closes_normally_and_relaunches()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::update::{attempt::AttemptOutcome, status::LaunchVersion};
    let mut runner = runner()?;
    runner.launch_update.finish(&AttemptOutcome::Prepared { version: "2.0.0".into() });
    runner.publish_update_status()?;
    assert_eq!(runner.update_status.snapshot().1.applying(), Some("2.0.0"));
    assert_eq!(runner.launch_version(), Some(LaunchVersion::Applying("2.0.0".into())));

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

/// When the window opens before the check prepares its update, nothing restarts: no close is
/// owed, the splash names no version, and a later close does not relaunch.
#[test]
fn an_update_prepared_after_the_window_opened_neither_closes_nor_relaunches()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::update::attempt::AttemptOutcome;
    let mut runner = runner()?;
    runner.launch_update.window_opened();
    runner.launch_update.finish(&AttemptOutcome::Prepared { version: "2.0.0".into() });
    runner.publish_update_status()?;
    assert_eq!(runner.update_status.snapshot().1.applying(), None);
    runner.initial_completed = true;
    runner.intake_ready = true;
    let owed = admission(&runner, true);
    assert!(!owed.requested);
    let fences = Cell::new(0);
    runner.close_after_fence(owed, |_| {
        fences.set(fences.get() + 1);
        Ok(true)
    })?;
    assert_eq!(fences.get(), 0);
    assert!(!runner.closing);
    runner.normal_shutdown = true;
    assert!(!runner.relaunch_wanted());
    Ok(())
}
