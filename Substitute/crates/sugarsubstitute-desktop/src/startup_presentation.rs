// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare covered child pixels before acknowledging an actual visible splash.
use super::*;
use sugarsubstitute_contracts::presentation::FrameShape;

/// Submit complete prepared pixels and controls while preserving the native show fence.
pub(super) fn submit(
    session: &mut Session,
    content: &mut impl StartupWindowContent,
    shape: Option<FrameShape>,
    timings: Option<&crate::timing::TimingObserver>,
) -> Result<(), DesktopError> {
    if session.splash.retiring {
        return Ok(());
    }
    let Some(frame) = session.splash.state.exposable(shape) else { return Ok(()) };
    let window = session
        .splash
        .window
        .as_ref()
        .ok_or_else(|| DesktopError::message("present_startup_splash", "window missing"))?;
    let presenter = session
        .splash
        .presenter
        .as_mut()
        .ok_or_else(|| DesktopError::message("present_startup_splash", "presenter missing"))?;
    let controls = session
        .splash
        .controls
        .as_mut()
        .ok_or_else(|| DesktopError::message("present_startup_splash", "controls missing"))?;
    session
        .splash
        .presentations
        .as_mut()
        .ok_or_else(|| {
            DesktopError::message("present_startup_splash", "presentation owner missing")
        })?
        .submit(window, presenter, frame, timings, content, controls)?;
    reconcile(session, content, timings)
}

/// Hide/show native presentation before forwarding its actual visibility observation.
pub(super) fn reconcile(
    session: &mut Session,
    content: &mut impl StartupWindowContent,
    timings: Option<&crate::timing::TimingObserver>,
) -> Result<(), DesktopError> {
    if session.splash.retiring || session.closing {
        return Ok(());
    }
    let window = session
        .splash
        .window
        .as_ref()
        .ok_or_else(|| DesktopError::message("reconcile_startup_splash", "window missing"))?;
    let presentations = session.splash.presentations.as_mut().ok_or_else(|| {
        DesktopError::message("reconcile_startup_splash", "presentation owner missing")
    })?;
    let presenter =
        session.splash.presenter.as_ref().ok_or_else(|| {
            DesktopError::message("reconcile_startup_splash", "presenter missing")
        })?;
    presentations.reconcile(content, window, presenter, timings)?;
    if presentations.shown()
        && (!session.splash.presented || session.splash.launch != content.launch_id())
    {
        session.splash.presented = true;
        session.splash.launch = content.launch_id();
        session
            .observe(content, DesktopStartupObservation::SplashPresented(session.splash.launch))?;
    }
    Ok(())
}
