// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A forwarding launch never activates its own server, so its pump admits the native loop when
//! the wait on the other copy must be shown, on the manual execution clock.
use super::super::tests::{NoChild, frozen_native_now};
use super::*;
use crate::{completion_wake::Wake, updates::test_support::Manual};
use std::sync::atomic::Ordering;
use sugarsubstitute_desktop::StartupPreparationStep;

const BOUND: NonZeroUsize = NonZeroUsize::MIN.saturating_add(31);

/// A runner with real execution on a manual clock, forwarding with no filesystem owner.
fn forwarding() -> Result<(EntryRunner<NoChild>, Arc<Manual>), Box<dyn Error>> {
    let root = PathBuf::from(r"E:\unopened-installation");
    let command = ProductionCommand {
        launch_selection: None,
        diagnostics: Default::default(),
        installation: root.clone(),
        options: sugarsubstitute_configuration::paths::StartupPaths::new(std::array::from_fn(
            |_| root.clone(),
        ))?
        .into(),
    };
    let mut runner = EntryRunner::new(
        command,
        root,
        Box::new(NoChild(None, None)),
        crate::diagnostics::DiagnosticHealth::fixture(),
    );
    let clock = Arc::new(Manual::default());
    runner.clock = clock.clone();
    runner.native_now = frozen_native_now;
    runner.initialize_execution(Arc::new(Wake::default()))?;
    runner.forward_wait = Some(wait::ForwardWait::new(runner.client()?.now()?)?);
    Ok((runner, clock))
}

/// Before two seconds the pump keeps preparing; at two seconds it admits its loop and the
/// waiting surface is handed over, where before it waited silently until the copy exited.
#[test]
fn a_forwarding_launch_admits_its_loop_when_the_wait_must_be_shown() -> Result<(), Box<dyn Error>> {
    let (mut runner, clock) = forwarding()?;
    clock.0.store(1_999_000_000, Ordering::SeqCst);
    assert_eq!(runner.prepare_native_runtime(BOUND)?, StartupPreparationStep::Waiting);
    assert!(runner.take_presentation().is_none());
    clock.0.store(2_000_000_000, Ordering::SeqCst);
    assert_eq!(runner.prepare_native_runtime(BOUND)?, StartupPreparationStep::Ready);
    assert!(runner.take_presentation().is_some(), "the waiting surface must be handed over");
    Ok(())
}

/// A launch whose forward was accepted never opens a loop for a surface.
#[test]
fn an_accepted_forward_never_admits_the_loop_for_a_surface() -> Result<(), Box<dyn Error>> {
    let (mut runner, clock) = forwarding()?;
    runner.initial_completed = true;
    clock.0.store(5_000_000_000, Ordering::SeqCst);
    assert_eq!(runner.prepare_native_runtime(BOUND)?, StartupPreparationStep::Waiting);
    Ok(())
}
