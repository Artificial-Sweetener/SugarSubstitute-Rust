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
