// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify the fixture waits themselves: a timer-held answer arrives on injected time, and an
//! answer nothing can deliver fails at once instead of waiting out the wall-clock backstop.

use super::*;
use viewport::fixture::Fixture;

/// Require `result` to fail as a deadlock naming `awaiting`.
fn deadlocked<T>(result: Result<T, Box<dyn Error>>, awaiting: &str) -> Result<(), Box<dyn Error>> {
    let Err(cause) = result else {
        return Err(
            format!("A wait for {awaiting} returned although nothing could deliver it").into()
        );
    };
    let message = cause.to_string();
    assert!(message.starts_with(&format!("Deadlocked waiting for {awaiting}:")), "{message}");
    Ok(())
}

#[test]
fn frames_owed_behind_the_camera_animation_timer_arrive_without_the_test_advancing_time()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let fitted = fixture.commit(4)?;
    fixture.viewport(5, json!({"type":"native"}))?;
    fixture.canvas.presented(fitted.frame)?;
    assert!(fixture.canvas.take_command().is_none());
    // The animation's first step is due only on its own timer, after every job has settled.
    let intermediate = fixture.pending()?;
    fixture.canvas.presented(intermediate)?;
    let reply = fixture.complete(5)?;
    assert_eq!(reply["viewport"]["mode"], "native");
    fixture.finish()
}

#[test]
fn a_frame_the_settled_canvas_never_prepares_fails_at_once_naming_it() -> Result<(), Box<dyn Error>>
{
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.settle()?;
    deadlocked(fixture.pending(), "a pending native frame")?;
    fixture.finish()
}

#[test]
fn a_reply_owed_only_to_native_acceptance_fails_at_once_naming_the_request()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.send(4, "surface.commit", json!({}))?;
    deadlocked(
        fixture.wait_reply(4, "applied"),
        "request 4 surface.commit to attachment:one at revision 4",
    )?;
    let target = fixture.pending()?;
    fixture.canvas.presented(target)?;
    fixture.take(4, "applied")?;
    fixture.finish()
}
