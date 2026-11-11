// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A candidate that arrives while another attachment still stages is early, never unavailable:
//! it is held, bounded by count, until native mutation frees, superseded by a newer candidate,
//! or withdrawn by its page.

use super::*;

const STAGED: Address<'static> = Address { workflow: "workflow", attachment: 2 };
const HELD: Address<'static> = Address { workflow: "workflow", attachment: 3 };
const NEWER: Address<'static> = Address { workflow: "workflow", attachment: 4 };

/// Stage `STAGED` over the committed attachment, leaving it prepared but not yet committed.
fn stage(host: &mut Host) -> Result<(), Box<dyn Error>> {
    commit_red(host)?;
    host.send_to(STAGED, 5, "surface.prepare", json!({}))?;
    host.expect(5, "ready")?;
    Ok(())
}

/// Finish the staged candidate: hidden geometry, a selection, visible geometry and its commit,
/// which promotes its document and frees native mutation.
fn commit_staged(host: &mut Host, first: u64) -> Result<(), Box<dyn Error>> {
    host.send_to(STAGED, first, "surface.geometry", geometry(80, 40, false))?;
    host.expect(first, "applied")?;
    host.send_to(STAGED, first + 1, "surface.select", select("tall.png"))?;
    host.expect(first + 1, "applied")?;
    host.send_to(STAGED, first + 2, "surface.geometry", geometry(80, 40, true))?;
    host.expect(first + 2, "applied")?;
    host.send_to(STAGED, first + 3, "surface.commit", json!({}))?;
    host.expect(first + 3, "applied")?;
    Ok(())
}

/// Let ten minutes of injected time pass with nothing able to progress.
fn wait_long(host: &mut Host) -> Result<(), Box<dyn Error>> {
    host.quiesce()?;
    host.fixture.advance(600_000);
    host.quiesce()
}

#[test]
fn a_candidate_that_arrives_while_another_stages_waits_for_it_without_a_deadline()
-> Result<(), Box<dyn Error>> {
    hosted(Host::new(0)?, |host| {
        stage(host)?;
        host.send_to(HELD, 6, "surface.prepare", json!({}))?;
        // Held, not refused: no reply, however long the host waits.
        wait_long(host)?;
        commit_staged(host, 7)?;
        // Native mutation is free, so the held candidate is prepared at last.
        answers(&host.expect(6, "ready")?, HELD, 6);
        host.send_to(HELD, 11, "surface.release", json!({}))?;
        host.expect(11, "applied")?;
        host.quiesce()?;
        idle(host)
    })
}

#[test]
fn a_newer_candidate_supersedes_the_held_one_and_is_prepared_in_its_place()
-> Result<(), Box<dyn Error>> {
    hosted(Host::new(0)?, |host| {
        stage(host)?;
        host.send_to(HELD, 6, "surface.prepare", json!({}))?;
        host.quiesce()?;
        host.send_to(NEWER, 7, "surface.prepare", json!({}))?;
        // At most one candidate is held: the older one is answered with a typed superseded outcome.
        answers(&host.expect(6, "stale")?, HELD, 6);
        wait_long(host)?;
        commit_staged(host, 8)?;
        answers(&host.expect(7, "ready")?, NEWER, 7);
        host.send_to(NEWER, 12, "surface.release", json!({}))?;
        host.expect(12, "applied")?;
        host.quiesce()?;
        idle(host)
    })
}

#[test]
fn a_held_candidate_its_page_withdraws_is_answered_cancelled_and_never_prepared()
-> Result<(), Box<dyn Error>> {
    hosted(Host::new(0)?, |host| {
        stage(host)?;
        host.send_to(HELD, 6, "surface.prepare", json!({}))?;
        host.quiesce()?;
        assert_eq!(host.cancel(6)?.as_deref(), Some("cancelled"));
        commit_staged(host, 7)?;
        host.quiesce()?;
        // The withdrawn candidate was never prepared, so its attachment has no surface to serve.
        host.send_to(HELD, 11, "surface.geometry", geometry(80, 40, true))?;
        host.expect(11, "stale")?;
        assert_eq!(host.shown(), Some(Shade::Blue));
        idle(host)
    })
}

#[test]
fn a_released_candidate_frees_the_next_prepare_and_is_never_rolled_back_for_it()
-> Result<(), Box<dyn Error>> {
    hosted(Host::new(0)?, |host| {
        stage(host)?;
        host.send_to(HELD, 6, "surface.prepare", json!({}))?;
        wait_long(host)?;
        // The staged candidate still owns the surface: a newer prepare never rolls it back.
        host.send_to(STAGED, 7, "surface.geometry", geometry(80, 40, false))?;
        host.expect(7, "applied")?;
        // Its page releases it explicitly, which frees native mutation for the held candidate.
        host.send_to(STAGED, 8, "surface.release", json!({}))?;
        host.expect(8, "applied")?;
        answers(&host.expect(6, "ready")?, HELD, 6);
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Red));
        host.send_to(HELD, 9, "surface.release", json!({}))?;
        host.expect(9, "applied")?;
        host.quiesce()?;
        idle(host)
    })
}
