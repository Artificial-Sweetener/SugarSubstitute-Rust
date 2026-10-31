// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Replay a closed and reopened output pane through staged reattachment and a later selection.
//! The host loop advances the injected clock only for deadlines the canvas actually reports.

use super::*;
use sugarsubstitute_desktop::canvas::acknowledgement::{Acknowledgement, FrameEffect, HideEffect};
use viewport::fixture::{Execution, Fixture};

/// Fake main-window compositor that decides every effect with the desktop's own acknowledgement
/// rules. Native custody is reduced to the active extent and displaced surfaces whose fence
/// completes at the start of the next UI turn, as the compositor's completion wake would.
#[derive(Default)]
struct Compositor {
    acknowledgement: Acknowledgement,
    active: Option<(u32, u32)>,
    retiring: bool,
}

impl Compositor {
    /// Present, retain or hide once per UI turn; report whether native custody progressed.
    fn dispatch(&mut self, fixture: &mut Fixture) -> Result<bool, Box<dyn Error>> {
        let fenced = std::mem::take(&mut self.retiring);
        let mut acknowledged = None;
        let mut committed = false;
        let mut hidden = false;
        match fixture.canvas.presentation() {
            MainCanvasPresentation::Frame(frame) => {
                let (target, placement) = (frame.content.target(), frame.placement);
                match self.acknowledgement.frame(
                    target,
                    placement,
                    frame.content.pending(),
                    self.retiring,
                )? {
                    FrameEffect::Committed(stamp) => acknowledged = stamp,
                    FrameEffect::AwaitRetirement => {}
                    FrameEffect::Commit => {
                        let extent = (placement.viewport.width, placement.viewport.height);
                        self.retiring = self.active.is_some_and(|active| active != extent);
                        self.active = Some(extent);
                        self.acknowledgement.committed(target, placement);
                        acknowledged = Some(target);
                    }
                }
            }
            MainCanvasPresentation::Retain(retained) => {
                if self.active.is_none()
                    || !self.acknowledgement.retains(retained.target, retained.placement)
                {
                    committed = self.hide().1;
                }
            }
            MainCanvasPresentation::Hide => (hidden, committed) = self.hide(),
        }
        if let Some(target) = acknowledged {
            fixture.canvas.presented(target)?;
        }
        if hidden {
            fixture.canvas.hidden(fixture.identity)?;
        }
        Ok(fenced || committed || acknowledged.is_some())
    }

    /// Return whether the hide completed and whether this turn committed a disconnection.
    fn hide(&mut self) -> (bool, bool) {
        match self.acknowledgement.hide(self.retiring) {
            HideEffect::Hidden => (true, false),
            HideEffect::AwaitRetirement => (false, false),
            HideEffect::Commit => {
                self.retiring |= self.active.take().is_some();
                self.acknowledgement.hide_committed();
                (false, true)
            }
        }
    }
}
/// When the page withdraws the first hidden-candidate selection relative to its worker progress.
#[derive(Clone, Copy, Debug)]
enum SelectionCancel {
    /// Before any completion was drained for the selection.
    Queued,
    /// After the selected source loaded but before its pixels were prepared.
    Loaded,
    /// After the host already replied; the cancellation only consumes its receipt.
    Replied,
}

/// Controllable orderings of the reopened pane's protocol record.
#[derive(Clone, Copy, Debug)]
struct ReopenOrder {
    /// Release the committed attachment before staging, or stage while it is still retained.
    release_first: bool,
    /// CSS viewport of the first hidden candidate geometries; a zero extent is undrawable.
    hidden: (u32, u32),
    cancel: SelectionCancel,
    /// Drain all refinement and retirement work after the staged commit before the next request.
    quiesce: bool,
    /// Injected page time in milliseconds before each request through the staged commit.
    page_delay: u64,
    /// Injected time before the viewport read and the user's later click.
    click_delay: u64,
}

/// Drive the application canvas the way the desktop event loop does, without real-time waits.
struct Replay {
    fixture: Fixture,
    native: Compositor,
    page_delay: u64,
}

impl Replay {
    /// Address the original attachment `1` or the reopened staged attachment by generation.
    fn send(
        &mut self,
        attachment: u64,
        id: u64,
        kind: &str,
        mut extra: Value,
    ) -> Result<(), Box<dyn Error>> {
        let name = if attachment == 1 { "one".to_owned() } else { attachment.to_string() };
        extra.as_object_mut().ok_or("Request fields absent")?.insert(
            "address".into(),
            json!({
                "sessionId":"canvas:workflow", "viewId":"view:one", "windowId":"window:one",
                "attachmentId":format!("attachment:{name}"), "generation":attachment
            }),
        );
        self.fixture.advance(self.page_delay);
        self.fixture.send(id, kind, extra)
    }

    /// Run one UI turn: drain completions, poll, then let native composition acknowledge.
    /// Returns a correlated reply, or whether this turn made observable progress.
    fn turn(&mut self, id: u64) -> Result<Result<Value, bool>, Box<dyn Error>> {
        let observed = self.fixture.wake.revision();
        self.fixture.pump()?;
        let progressed = self.native.dispatch(&mut self.fixture)?;
        if let Some(command) = self.fixture.canvas.take_command() {
            return Ok(Ok(surface_reply(command, id)?));
        }
        if progressed || self.fixture.wake.revision() != observed {
            return Ok(Err(true));
        }
        let execution = self.fixture.execution()?;
        if execution == Execution::Running {
            self.fixture.wake.wait_after(observed)?;
            return Ok(Err(true));
        }
        if self.fixture.canvas.next_deadline().is_some() {
            self.fixture.advance(100);
            return Ok(Err(true));
        }
        if execution == Execution::Delivering {
            self.fixture.wake.wait_after(observed)?;
            return Ok(Err(true));
        }
        // A correlated request is still owed a reply: worker-side releases can arm a capacity
        // wake after every job settled, so only that wake's absence proves a stall.
        if id != 0 && self.fixture.wake.wait_after(observed).is_ok() {
            return Ok(Err(true));
        }
        Ok(Err(false))
    }

    /// Settle a request; an idle owner with no deadline and no reply is a stall.
    fn expect(&mut self, id: u64, status: &str) -> Result<Value, Box<dyn Error>> {
        for _ in 0..4096 {
            match self.turn(id)? {
                Ok(value) if value["status"] == status => return Ok(value),
                Ok(value) => {
                    return Err(
                        format!("Request {id} replied {} not {status}", value["status"]).into()
                    );
                }
                Err(true) => {}
                Err(false) => {
                    return Err(
                        format!("Request {id} stalled: no reply, worker or deadline").into()
                    );
                }
            }
        }
        Err(format!("Request {id} kept progressing without a reply").into())
    }

    /// Present every refinement and retire all work until no owner can make progress.
    fn quiesce(&mut self) -> Result<(), Box<dyn Error>> {
        for _ in 0..4096 {
            match self.turn(0)? {
                Ok(value) => return Err(format!("Unexpected idle reply {value}").into()),
                Err(true) => {}
                Err(false) => return Ok(()),
            }
        }
        Err("Canvas kept progressing without becoming idle".into())
    }

    /// Progress a pending selection until its source loaded; report false if it replied first.
    fn until_loaded(&mut self, id: u64) -> Result<bool, Box<dyn Error>> {
        for _ in 0..4096 {
            let loaded = self.fixture.canvas.session.as_ref().is_some_and(|session| {
                matches!(
                    session.camera_intent(),
                    crate::native_canvas::session::CameraCommandResult::Accepted(_)
                )
            });
            if loaded {
                return Ok(true);
            }
            match self.turn(id)? {
                Ok(value) if value["status"] == "applied" => return Ok(false),
                Ok(value) => return Err(format!("Selection replied {}", value["status"]).into()),
                Err(true) => {}
                Err(false) => return Err(format!("Selection {id} stalled before loading").into()),
            }
        }
        Err("Selection kept progressing without loading".into())
    }

    /// Cancel through the separate lane, returning any ordinary terminal it produced.
    fn cancel(&mut self, id: u64) -> Result<Option<String>, Box<dyn Error>> {
        self.fixture.cancel(id)?;
        let mut status = None;
        loop {
            match self.fixture.canvas.take_command() {
                Some(DesktopStartupCommand::SendSurface(reply)) => {
                    let value: Value = serde_json::from_slice(reply.payload())?;
                    assert_eq!(value["requestId"], id);
                    status = value["status"].as_str().map(str::to_owned);
                }
                Some(DesktopStartupCommand::CompletePanelIngress(receipt)) => {
                    assert_eq!(receipt.identity(), self.fixture.identity);
                    return Ok(status);
                }
                Some(_) => return Err("Cancellation produced an unrelated command".into()),
                None => return Err("Cancellation consumption receipt is missing".into()),
            }
        }
    }
}

fn surface_reply(command: DesktopStartupCommand, id: u64) -> Result<Value, Box<dyn Error>> {
    let DesktopStartupCommand::SendSurface(reply) = command else {
        return Err("Expected an ordinary surface reply".into());
    };
    let value: Value = serde_json::from_slice(reply.payload())?;
    if value["requestId"] != id {
        return Err(format!("Expected reply {id}, received {value}").into());
    }
    Ok(value)
}

fn geometry(width: u32, height: u32, visible: bool) -> Value {
    let clip = if visible { (width, height) } else { (0, 0) };
    json!({"geometry":{
        "coordinateSpace":"client", "units":"css-pixels", "origin":{"x":10,"y":20},
        "viewport":{"width":width,"height":height},
        "clip":{"x":10,"y":20,"width":clip.0,"height":clip.1},
        "physicalPixelsPerCssPixel":1, "visible":visible, "focused":false
    }})
}

fn select(filename: &str) -> Value {
    json!({
        "itemId":format!("result:{filename}"),
        "file":{"filename":filename,"subfolder":"","type":"output"}
    })
}

/// Replay the recorded reopened-pane sequence and require the final selection to present B.
fn replay(order: ReopenOrder) -> Result<(), Box<dyn Error>> {
    let mut replay = Replay {
        fixture: Fixture::new()?,
        native: Compositor::default(),
        page_delay: order.page_delay,
    };
    let result = reopen_and_select(&mut replay, order);
    while replay.fixture.canvas.take_command().is_some() {}
    let finished = replay.fixture.finish();
    result.and(finished)
}

fn reopen_and_select(replay: &mut Replay, order: ReopenOrder) -> Result<(), Box<dyn Error>> {
    replay.send(1, 4, "surface.commit", json!({}))?;
    replay.expect(4, "applied")?;
    replay.quiesce()?;
    if order.release_first {
        replay.send(1, 5, "surface.release", json!({}))?;
        replay.expect(5, "applied")?;
        replay.send(4, 6, "surface.prepare", json!({}))?;
        replay.expect(6, "ready")?;
    } else {
        replay.send(4, 5, "surface.prepare", json!({}))?;
        replay.expect(5, "ready")?;
        replay.send(1, 6, "surface.release", json!({}))?;
        replay.expect(6, "stale")?;
    }
    let (width, height) = order.hidden;
    for id in [9, 10] {
        replay.send(4, id, "surface.geometry", geometry(width, height, false))?;
        replay.expect(id, "applied")?;
        assert_eq!(replay.cancel(id)?, None);
    }
    replay.send(4, 11, "surface.geometry", geometry(width, height, false))?;
    replay.expect(11, "applied")?;
    replay.send(4, 12, "surface.select", select("wide.png"))?;
    let pending = match order.cancel {
        SelectionCancel::Queued => true,
        SelectionCancel::Loaded => replay.until_loaded(12)?,
        SelectionCancel::Replied => {
            replay.expect(12, "applied")?;
            false
        }
    };
    let cancelled = replay.cancel(12)?;
    assert_eq!(cancelled.as_deref(), pending.then_some("cancelled"));
    let (width, height) = if width == 0 || height == 0 { (80, 40) } else { (width, height) };
    replay.send(4, 13, "surface.geometry", geometry(width, height, false))?;
    replay.expect(13, "applied")?;
    replay.send(4, 14, "surface.select", select("wide.png"))?;
    replay.expect(14, "applied")?;
    replay.send(4, 15, "surface.geometry", geometry(80, 40, true))?;
    replay.expect(15, "applied")?;
    replay.send(4, 16, "surface.commit", json!({}))?;
    replay.expect(16, "applied")?;
    if order.quiesce {
        replay.quiesce()?;
    }
    replay.page_delay = order.click_delay;
    replay.send(4, 17, "surface.viewport.request", json!({"command":{"type":"read"}}))?;
    replay.expect(17, "applied")?;
    replay.send(4, 18, "surface.select", select("tall.png"))?;
    replay.expect(18, "applied")?;
    let accepted = replay.fixture.accepted()?;
    assert_eq!(accepted.extent, sugarsubstitute_image_ops::Extent::new(160, 320)?);
    let shown = match replay.fixture.canvas.presentation() {
        MainCanvasPresentation::Frame(frame) if !frame.content.pending() => frame.content.target(),
        MainCanvasPresentation::Retain(retained) => retained.target,
        _ => return Err("The final selection is not the acknowledged native presentation".into()),
    };
    assert_eq!(shown, accepted.frame);
    replay.quiesce()
}

/// Cover drawable and undrawable hidden candidates, first-selection cancellation points and
/// post-commit settlement. Requests arrive back to back, as recorded, or one display period apart.
fn orderings(release_first: bool) -> Result<(), Box<dyn Error>> {
    let mut failures = Vec::new();
    for hidden in [(0, 0), (80, 40)] {
        for cancel in [SelectionCancel::Queued, SelectionCancel::Loaded, SelectionCancel::Replied] {
            // An undrawable candidate never prepares pixels, so only the page can retire it.
            if hidden == (0, 0) && matches!(cancel, SelectionCancel::Replied) {
                continue;
            }
            for quiesce in [false, true] {
                for (page_delay, click_delay) in [(0, 0), (100, 100)] {
                    let order = ReopenOrder {
                        release_first,
                        hidden,
                        cancel,
                        quiesce,
                        page_delay,
                        click_delay,
                    };
                    if let Err(cause) = replay(order) {
                        failures.push(format!("{order:?}: {cause}"));
                    }
                }
            }
        }
    }
    if failures.is_empty() { Ok(()) } else { Err(failures.join("\n").into()) }
}

#[test]
fn reopened_pane_presents_a_later_selection_after_releasing_before_staging()
-> Result<(), Box<dyn Error>> {
    orderings(true)
}

#[test]
fn reopened_pane_presents_a_later_selection_while_the_old_binding_is_retained()
-> Result<(), Box<dyn Error>> {
    orderings(false)
}
