// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify concurrent, stale, cancelled, reloaded, malformed and failed canvas work against the
//! real application canvas, its execution owner and the desktop acknowledgement rules.

use super::*;
use boundary::Execution;
use gate::Gate;
use host::{Address, Host, Shade, geometry, select};
use viewport::fixture::Fixture;

mod deferral;
mod ingress;
mod loads;
mod ordering;
mod reload;
mod shutdown;
mod stale;
mod withdrawal;
mod workflows;

/// Worker progress a selection has reached when the page withdraws or supersedes it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stage {
    /// A worker is blocked inside the host acquisition.
    Acquire,
    /// The acquisition returned; its decode and completion are not yet observed.
    Load,
    /// The decoded source was observed; its pixels are not yet observed.
    Render,
    /// Pixels are prepared and await native acknowledgement on a committed attachment.
    Present,
}

/// Select `filename` at `address` and hold the selection at `stage`. Acquisition stages use a
/// gate, `cooperative` when it stops on cancellation.
fn select_at(
    host: &mut Host,
    address: Address<'_>,
    id: u64,
    filename: &str,
    stage: Stage,
    cooperative: bool,
) -> Result<Option<Arc<Gate>>, Box<dyn Error>> {
    let gate =
        matches!(stage, Stage::Acquire | Stage::Load).then(|| host.gate(filename, cooperative));
    host.send_to(address, id, "surface.select", select(filename))?;
    if let Some(gate) = &gate {
        host.pump_until("acquisition", |_| gate.entered())?;
    }
    match stage {
        Stage::Acquire => {}
        Stage::Load => {
            let gate = gate.as_ref().ok_or("Load stage lost its gate")?;
            gate.release();
            // Wait without pumping, so the loading completion remains unobserved.
            while !gate.returned() {
                let observed = host.fixture.wake.revision();
                if !gate.returned() {
                    host.fixture.wake.wait_after(observed);
                }
            }
        }
        Stage::Render => host.pump_until("load", loaded)?,
        Stage::Present => host.pump_until("prepared frame", |fixture| {
            prepared_frame(&fixture.canvas).is_some_and(|frame| frame.content.pending())
        })?,
    }
    Ok(gate)
}

/// Report that the selected source's load completion was observed and its camera exists.
fn loaded(fixture: &Fixture) -> bool {
    fixture.canvas.session.as_ref().is_some_and(|session| {
        matches!(
            session.camera_intent(),
            crate::native_canvas::session::CameraCommandResult::Accepted(_)
        )
    })
}

/// Require a reply to answer exactly the request sent to `address` at `revision`.
fn answers(reply: &Value, address: Address<'_>, revision: u64) {
    assert_eq!(reply["address"]["sessionId"], format!("canvas:{}", address.workflow));
    assert_eq!(reply["address"]["generation"], address.attachment);
    assert_eq!(reply["revision"], revision);
    assert_eq!(reply["requestId"], revision);
}

/// Require that no admitted job, undelivered completion or request credit remains.
fn idle(host: &mut Host) -> Result<(), Box<dyn Error>> {
    assert_eq!(host.fixture.execution()?, Execution::Idle);
    drop(host.fixture.memory.claim(host.fixture.memory.bytes())?);
    Ok(())
}

/// Run `body` and retire the host's scope even when the body fails.
fn hosted(
    mut host: Host,
    body: impl FnOnce(&mut Host) -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let result = body(&mut host);
    let finished = host.finish();
    result.and(finished)
}

/// Commit the fixture's selected wide red output and wait until composition shows it.
fn commit_red(host: &mut Host) -> Result<(), Box<dyn Error>> {
    host.send(1, 4, "surface.commit", json!({}))?;
    host.expect(4, "applied")?;
    host.quiesce()?;
    assert_eq!(host.shown(), Some(Shade::Red));
    Ok(())
}

/// Decode a page's discovery-time context request through its public contract.
fn context(panel: &PanelSession, id: u64) -> Result<CanvasNativeRequest, Box<dyn Error>> {
    Ok(CanvasNativeRequest::decode(&serde_json::to_vec(&json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":panel.as_str(),
        "requestId":id, "type":"context.request"
    }))?)?)
}

/// Collect every failed ordering instead of stopping at the first.
fn all<T: std::fmt::Debug + Copy>(
    cases: impl IntoIterator<Item = T>,
    run: impl Fn(T) -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let failures: Vec<_> = cases
        .into_iter()
        .filter_map(|case| run(case).err().map(|cause| format!("{case:?}: {cause}")))
        .collect();
    if failures.is_empty() { Ok(()) } else { Err(failures.join("\n").into()) }
}
