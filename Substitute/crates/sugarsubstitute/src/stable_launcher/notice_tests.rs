// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The notice appears only after two seconds on the injected clock, prepares frames on the
//! launcher's executor, and never affects startup when it fails.
use super::*;
use crate::{
    job_sequence::JobSequence,
    updates::test_support::{Manual, Wake, observe},
};
use std::sync::Arc;
use sugarsubstitute_contracts::{
    execution::{Clock, ScopeId, ScopeSpec},
    presentation::FrameShape,
};
use sugarsubstitute_execution::ExecutionOwner;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

/// One drain budget for the fixture's completion turns.
const DRAIN: std::num::NonZeroUsize = std::num::NonZeroUsize::MIN.saturating_add(31);

fn at(millis: u64) -> Moment {
    Moment::from_nanos(millis * 1_000_000)
}

fn notice() -> Result<Notice> {
    Ok(Notice::new(LauncherBudget::new(at(0))?))
}

#[test]
fn a_fast_launch_never_asks_for_a_window() -> Result {
    let mut notice = notice()?;
    assert!(notice.next_wake().is_some(), "the pump wakes at the notice moment");
    notice.observe(at(1_999), true);
    assert!(!notice.shown());
    assert!(notice.wanted().is_none());
    notice.observe(at(1_999), false);
    assert!(notice.next_wake().is_none(), "a handed-off launch needs no wake");
    notice.observe(at(5_000), true);
    assert!(notice.wanted().is_none(), "it never appears after the hand-off");
    assert!(!notice.busy());
    Ok(())
}

#[test]
fn a_slow_launch_asks_for_the_splash_shaped_notice_until_it_hands_off() -> Result {
    let mut notice = notice()?;
    notice.observe(at(2_000), true);
    assert!(notice.shown());
    assert!(notice.next_wake().is_none());
    let wanted = notice.wanted().ok_or("notice not wanted")?;
    assert_eq!(wanted.title, crate::APPLICATION_NAME);
    assert_eq!(wanted.logical_size, [LOGICAL_SIZE; 2]);
    assert!(wanted.frame.is_none(), "no frame before the pump reports its surface");
    notice.observe(at(9_000), false);
    assert!(!notice.shown());
    assert!(notice.wanted().is_none(), "the hand-off closes it");
    Ok(())
}

/// The notice's own appearance: the splash, saying it is starting, without controls.
#[test]
fn the_notice_is_the_splash_saying_it_is_starting_without_controls() -> Result {
    let surface =
        NoticeSurface { shape: FrameShape::new(837, 837, 1.5)?, dark: false, accent: None };
    let request = request(surface)?;
    assert_eq!(request.shape, surface.shape);
    assert_eq!(request.appearance.status, StartupStatus::Starting);
    assert!(!request.appearance.controls);
    assert!(!request.appearance.expanded);
    assert_eq!(request.appearance.theme, Theme::Light);
    assert!(request.console.is_none());
    Ok(())
}

/// A frame that cannot be prepared ends the notice quietly: it is no longer wanted, and its
/// jobs have ended, so the launcher's executor can still settle.
#[test]
fn a_notice_that_cannot_be_prepared_ends_without_affecting_startup() -> Result {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let resources = Resources { client: &client, scope: &scope, jobs: &jobs, until: at(120_000) };
    let mut notice = notice()?;
    notice.observe(at(2_000), true);
    // Below the splash's smallest scale, so preparation refuses the frame without drawing.
    notice.surface(NoticeSurface { shape: FrameShape::new(1, 1, 0.25)?, dark: true, accent: None });
    notice.advance(&resources, false);
    assert!(notice.busy(), "renderer initialization was admitted");
    observe(&wake, || {
        client.drain_completions(DRAIN)?;
        notice.advance(&resources, false);
        Ok(!notice.busy() && !notice.shown())
    })?;
    assert!(notice.wanted().is_none());
    assert!(jobs.allocate()?.get() >= 3, "initialization and preparation both ran");
    owner.request_shutdown(clock.now().checked_add(LauncherBudget::EXIT_GRACE)?);
    observe(&wake, || Ok(owner.poll_shutdown()?.settled))?;
    owner.finish_shutdown()?;
    Ok(())
}

/// Once the pump reports its window, one frame is prepared for exactly that surface and handed
/// over once. The frame draws the status words, so this needs the platform's text renderer.
#[test]
fn a_shown_notice_prepares_one_frame_for_the_reported_window() -> Result {
    let clock = Arc::new(Manual::default());
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let resources = Resources { client: &client, scope: &scope, jobs: &jobs, until: at(120_000) };
    let mut notice = notice()?;
    notice.observe(at(2_000), true);
    let shape = FrameShape::new(558, 558, 1.0)?;
    notice.surface(NoticeSurface { shape, dark: true, accent: None });
    observe(&wake, || {
        client.drain_completions(DRAIN)?;
        notice.advance(&resources, false);
        Ok(!notice.busy())
    })?;
    let frame = notice.wanted().and_then(|wanted| wanted.frame).ok_or("no frame prepared")?;
    assert_eq!(frame.shape(), shape);
    assert!(notice.wanted().is_some_and(|wanted| wanted.frame.is_none()), "handed over once");
    notice.advance(&resources, false);
    assert!(!notice.busy(), "one frame per window");
    owner.request_shutdown(clock.now().checked_add(LauncherBudget::EXIT_GRACE)?);
    observe(&wake, || Ok(owner.poll_shutdown()?.settled))?;
    owner.finish_shutdown()?;
    Ok(())
}
