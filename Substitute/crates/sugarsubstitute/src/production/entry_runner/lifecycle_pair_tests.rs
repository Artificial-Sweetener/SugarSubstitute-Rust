// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Restart, close and cancel flows end to end: the real entry runner and the window's real
//! startup host, each on its own thread, exchanging lifecycle frames.
//!
//! The coordinator peer is the real [`EntryRunner`] with its real update status, restart
//! decision, "Updating to version X…" notice, normal close, cleanup cutoff and relaunch, the
//! real per-desktop [`ParentLifecycle`] actor with its real desktop exit transaction, and, where
//! a scenario needs one, a real [`CoordinatorBackend`] SugarCubes owner. Its activation adapter
//! is the part of the production activation that has no native process to stand on: it keeps
//! one desktop's actor live until that desktop's process exit, then keeps what the production
//! activation's normal retirement keeps (whether the close completed normally and whether it
//! asked to restart), takes the backend's SugarCubes results every turn and reports the
//! activation settled once no desktop is live and the backend settled. The pump that would draw
//! the coordinator's notice window gives it a surface and takes its frames. The entry server
//! has no headless peer, so its idle fence answers idle and its intake counts as settled.
//! A relaunch is observed when the runner admits it; its launch targets an installation folder
//! holding no executable, so no process is ever created.
use super::*;
use crate::{
    lifecycle_harness::{HarnessResult, Log, Pair, PeerThread, pipe},
    startup::lifecycle_peer::{WindowPeer, WindowStart, WindowState},
};
use sugarsubstitute_contracts::lifecycle::CompletionIntent;
#[path = "lifecycle_pair_tests/activation.rs"]
mod activation;
#[path = "lifecycle_pair_tests/clock.rs"]
mod clock;
#[path = "lifecycle_pair_tests/coordinator.rs"]
mod coordinator;
use coordinator::{CoordinatorPeer, CoordinatorState};

type Peers = Pair<WindowPeer, CoordinatorPeer>;

/// Start both peers: the window as `start` says, the coordinator with that window's desktop.
fn pair(start: WindowStart) -> HarnessResult<Peers> {
    let log = Log::default();
    let (window_end, coordinator_end, in_flight) = pipe(&log);
    let mut window = PeerThread::spawn("lifecycle-window", move || {
        WindowPeer::new(window_end, start).map_err(|cause| cause.to_string())
    })?;
    let wake = window.call(|peer| peer.wake())?;
    let (run, launch) = window.call(|peer| peer.run_and_launch())?;
    let shared = wake.clone();
    let coordinator = PeerThread::spawn("lifecycle-coordinator", move || {
        CoordinatorPeer::new(coordinator_end, shared, run, launch)
            .map_err(|cause| cause.to_string())
    })?;
    Ok(Pair::new(window, coordinator, wake, in_flight, log))
}

fn window(pair: &mut Peers) -> HarnessResult<WindowState> {
    pair.window.call(|peer| peer.state())
}
fn coordinator(pair: &mut Peers) -> HarnessResult<CoordinatorState> {
    pair.coordinator.call(|peer| peer.state())
}

/// Fail with `what` and the exchange log unless `holds`.
fn expect(pair: &Peers, holds: bool, what: &str) -> HarnessResult {
    if holds {
        return Ok(());
    }
    Err(format!("expected: {what}\nexchange log:\n{}", pair.log().render()).into())
}

/// Whether the coordinator is done with this run: it exited, or it already cut the window off.
fn settled(pair: &mut Peers) -> HarnessResult<bool> {
    let state = coordinator(pair)?;
    Ok(state.finished || state.window_cut_off)
}

/// Let the pair settle and check the coordinator closed without any cutoff or failure.
fn expect_clean_exit(pair: &mut Peers) -> HarnessResult<CoordinatorState> {
    pair.run_until("the coordinator exits", |pair| Ok(coordinator(pair)?.finished))?;
    let state = coordinator(pair)?;
    expect(pair, !state.window_cut_off, "the coordinator never cut the window off")?;
    expect(pair, !state.cutoff_failure, "no cleanup cutoff fired")?;
    expect(pair, state.failures.is_empty(), &format!("no failure: {:?}", state.failures))?;
    Ok(state)
}

/// "Restart to apply": the notice is owed and shown, then ExitIntent, RestartToApply and
/// ExitCompleted, and the relaunch, without the window ever being cut off at its grace.
#[test]
fn restart_to_apply_shows_the_notice_completes_the_close_and_relaunches() -> HarnessResult {
    let mut pair = pair(WindowStart::Revealed)?;
    pair.run_until("the coordinator learns the window opened", |pair| {
        Ok(coordinator(pair)?.window_opened)
    })?;
    pair.window.act(WindowPeer::click_restart_to_apply)?;
    pair.run_until("the run restarts or ends", |pair| {
        let state = coordinator(pair)?;
        Ok(state.relaunch_requested || settled(pair)?)
    })?;
    let window = window(&mut pair)?;
    expect(&pair, window.exit_intent == Some(CompletionIntent::Normal), "a normal exit intent")?;
    expect(&pair, window.restart_sent, "RestartToApply was sent")?;
    expect(&pair, window.completed && window.exited, "the window completed its close and exited")?;
    expect(&pair, !window.restart_held && !window.terminated, "the window was never cut off")?;
    let log = pair.log().render();
    let shown = log.find("the Updating notice is on screen");
    let completed = log.find("ExitCompleted(1)");
    expect(
        &pair,
        shown.is_some() && shown < completed,
        "the notice is on screen before ExitCompleted",
    )?;
    let state = expect_clean_exit(&mut pair)?;
    expect(&pair, state.relaunch_requested, "restart_to_apply_relaunch requested")
}

/// A restart decided before the window opened: the splash says "Updating to version 2.0.0…",
/// stays until the coordinator's notice covers the screen, then the window closes itself and
/// the coordinator relaunches.
#[test]
fn an_eager_restart_before_the_window_opens_holds_the_splash_then_relaunches() -> HarnessResult {
    let mut pair = pair(WindowStart::Loading)?;
    pair.coordinator.act(CoordinatorPeer::decide_restart)?;
    pair.run_until("the splash learns the restart", |pair| Ok(window(pair)?.restarting_status))?;
    let held = window(&mut pair)?;
    expect(
        &pair,
        held.update_line.as_deref() == Some("Updating to version 2.0.0"),
        &format!("the splash shows Updating to version 2.0.0: {:?}", held.update_line),
    )?;
    expect(&pair, !held.closing || held.restart_covered_status, "the splash holds until covered")?;
    pair.run_until("the run restarts or ends", |pair| {
        let state = coordinator(pair)?;
        Ok(state.relaunch_requested || settled(pair)?)
    })?;
    let window = window(&mut pair)?;
    expect(&pair, !window.revealed, "the window was never revealed")?;
    expect(&pair, window.completed && window.exited, "the window closed itself and exited")?;
    let log = pair.log().render();
    let shown = log.find("the Updating notice is on screen");
    let intent = log.find("ExitIntent");
    expect(
        &pair,
        shown.is_some() && shown < intent,
        "the notice covers the screen before the window closes",
    )?;
    let state = expect_clean_exit(&mut pair)?;
    expect(&pair, state.relaunch_requested, "restart_to_apply_relaunch requested")
}

/// A window revealed before it processed the restart status is never closed by the coordinator:
/// its MainRevealed withdraws the restart, and the window stays open.
#[test]
fn a_window_revealed_before_the_restart_status_withdraws_the_restart() -> HarnessResult {
    let mut pair = pair(WindowStart::Loading)?;
    pair.coordinator.act(CoordinatorPeer::decide_restart)?;
    pair.window.act(WindowPeer::reveal)?;
    pair.run_until("the coordinator withdraws the restart", |pair| {
        let state = coordinator(pair)?;
        Ok(state.window_opened && !state.restart_decided && !state.status_restarting)
    })?;
    pair.run_until("the window learns the restart was withdrawn", |pair| {
        Ok(!window(pair)?.restarting_status)
    })?;
    pair.run_for("the window keeps running", Duration::from_secs(30))?;
    let window = window(&mut pair)?;
    let state = coordinator(&mut pair)?;
    expect(&pair, !window.closing && window.exit_intent.is_none(), "the window is still open")?;
    expect(&pair, state.has_desktop && !state.closing, "the coordinator never closed the window")?;
    expect(&pair, !state.relaunch_requested, "nothing relaunches")
}

/// A restart decided while a SugarCubes job puts files back waits for it, never fails on its
/// cleanup cutoff, and relaunches once it finished.
#[test]
fn a_restart_during_a_sugarcubes_install_waits_for_it_then_relaunches() -> HarnessResult {
    let mut pair = pair(WindowStart::Loading)?;
    pair.coordinator.act(|peer| peer.start_sugarcubes_job(true))?;
    pair.coordinator.act(CoordinatorPeer::decide_restart)?;
    pair.run_for("the SugarCubes job keeps running", Duration::from_secs(20))?;
    let waiting = coordinator(&mut pair)?;
    expect(
        &pair,
        !waiting.window_cut_off && !waiting.cutoff_failure,
        "nothing is cut off while it runs",
    )?;
    expect(
        &pair,
        waiting.relaunch_requested || !waiting.finished,
        "the restart waits for the job",
    )?;
    pair.coordinator.call(CoordinatorPeer::release_install)?;
    pair.run_until("the run restarts or ends", |pair| {
        let state = coordinator(pair)?;
        Ok(state.relaunch_requested || state.finished)
    })?;
    let state = expect_clean_exit(&mut pair)?;
    expect(&pair, state.relaunch_requested, "restart_to_apply_relaunch requested")
}

/// A coordinator close of a live window asks it to close; it closes through its close save,
/// ExitIntent and ExitCompleted, and the coordinator then cleans up and exits.
#[test]
fn a_coordinator_close_lets_the_window_close_normally_then_cleans_up() -> HarnessResult {
    let mut pair = pair(WindowStart::Revealed)?;
    pair.run_until("the coordinator learns the window opened", |pair| {
        Ok(coordinator(pair)?.window_opened)
    })?;
    pair.coordinator.act(CoordinatorPeer::close)?;
    pair.run_until("the window is gone", |pair| {
        let window = window(pair)?;
        Ok(window.exited || window.terminated)
    })?;
    let window = window(&mut pair)?;
    expect(&pair, pair.log().contains("CloseRequest"), "the coordinator sent CloseRequest")?;
    expect(
        &pair,
        window.close_reason == "Close",
        &format!("an ordinary close: {}", window.close_reason),
    )?;
    expect(&pair, window.close_saved.is_some(), "the close save has its outcome")?;
    expect(&pair, window.exit_intent == Some(CompletionIntent::Normal), "a normal exit intent")?;
    expect(&pair, window.completed && !window.terminated, "the window completed its close")?;
    let state = expect_clean_exit(&mut pair)?;
    expect(&pair, !state.relaunch_requested, "a close relaunches nothing")
}

/// "Cancel loading" with a backend whose cleanup failed while a SugarCubes job finished: the
/// coordinator exits once its last desktop is gone.
#[test]
fn cancel_loading_with_a_failed_backend_cleanup_exits_once_the_desktop_is_gone() -> HarnessResult {
    let mut pair = pair(WindowStart::Loading)?;
    pair.coordinator.act(|peer| peer.start_sugarcubes_job(false))?;
    pair.window.act(WindowPeer::cancel_loading)?;
    pair.run_until("the coordinator accepts the cancelled startup", |pair| {
        Ok(window(pair)?.intent_acknowledged)
    })?;
    pair.coordinator.call(CoordinatorPeer::fail_backend_cleanup)?;
    pair.run_until("the coordinator exits", settled)?;
    let window = window(&mut pair)?;
    let state = coordinator(&mut pair)?;
    expect(
        &pair,
        window.exit_intent == Some(CompletionIntent::StartupCancelled),
        "a cancelled startup",
    )?;
    expect(&pair, window.exited && !window.terminated, "the window exited by itself")?;
    expect(
        &pair,
        state.finished && !state.has_desktop,
        "the coordinator exited with no desktop left",
    )?;
    expect(&pair, !state.relaunch_requested, "a cancel relaunches nothing")
}
