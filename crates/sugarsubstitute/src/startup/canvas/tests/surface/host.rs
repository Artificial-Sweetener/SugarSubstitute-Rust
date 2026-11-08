// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the application canvas the way the desktop event loop does, without real-time waits.
//! The host loop advances the injected clock only for deadlines the canvas actually reports.

use super::*;
use gate::Gate;
use sugarsubstitute_canvas::FrameStamp;
use sugarsubstitute_desktop::canvas::acknowledgement::{Acknowledgement, FrameEffect, HideEffect};
use viewport::fixture::{Execution, Fixture};

/// Dominant colour of the fixture image a committed native frame shows at its centre.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Shade {
    Red,
    Green,
    Blue,
    Other,
}

impl Shade {
    /// Classify the centre pixel of row-major `pixels` with `width` columns.
    fn of(pixels: &[u32], width: u32) -> Self {
        let width = usize::try_from(width).unwrap_or(usize::MAX).max(1);
        let rows = pixels.len() / width;
        let Some(&pixel) = pixels.get((rows / 2) * width + width / 2) else {
            return Self::Other;
        };
        let level = |shift: u32| match (pixel >> shift) & 0xff {
            225.. => Some(true),
            ..32 => Some(false),
            _ => None,
        };
        match (level(16), level(8), level(0)) {
            (Some(true), Some(false), Some(false)) => Self::Red,
            (Some(false), Some(true), Some(false)) => Self::Green,
            (Some(false), Some(false), Some(true)) => Self::Blue,
            _ => Self::Other,
        }
    }
}

/// Fake main-window compositor that decides every effect with the desktop's own acknowledgement
/// rules. Native custody is reduced to the active extent and displaced surfaces whose fence
/// completes at the start of the next UI turn, as the compositor's completion wake would.
#[derive(Default)]
struct Compositor {
    acknowledgement: Acknowledgement,
    active: Option<(u32, u32)>,
    retiring: bool,
    /// Shade of every frame committed to the native tree, in commit order.
    committed: Vec<(FrameStamp, Shade)>,
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
                        self.committed.push((
                            target,
                            Shade::of(frame.content.cpu_pixels(), placement.viewport.width),
                        ));
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

/// One page-side attachment address: a SugarCubes workflow session and attachment generation.
#[derive(Clone, Copy, Debug)]
pub(super) struct Address<'a> {
    pub(super) workflow: &'a str,
    pub(super) attachment: u64,
}

impl Address<'_> {
    /// The attachment the fixture prepared for the default workflow.
    pub(super) const ORIGINAL: Address<'static> = Address { workflow: "workflow", attachment: 1 };

    fn wire(self) -> Value {
        let name =
            if self.attachment == 1 { "one".to_owned() } else { self.attachment.to_string() };
        json!({
            "sessionId":format!("canvas:{}", self.workflow), "viewId":"view:one",
            "windowId":"window:one", "attachmentId":format!("attachment:{name}"),
            "generation":self.attachment
        })
    }
}

/// The real canvas, its execution owner and a fake compositor driven one UI turn at a time.
pub(super) struct Host {
    pub(super) fixture: Fixture,
    native: Compositor,
    /// Injected page time in milliseconds before each request.
    pub(super) page_delay: u64,
}

impl Host {
    /// Start from the fixture's prepared visible attachment with `wide.png` selected.
    pub(super) fn new(page_delay: u64) -> Result<Self, Box<dyn Error>> {
        Ok(Self { fixture: Fixture::new()?, native: Compositor::default(), page_delay })
    }

    /// Start from a prepared visible empty attachment, before any source or native pixels.
    pub(super) fn empty() -> Result<Self, Box<dyn Error>> {
        Ok(Self { fixture: Fixture::empty()?, native: Compositor::default(), page_delay: 0 })
    }

    /// Start from a prepared visible attachment whose selections fetch from `backend`.
    pub(super) fn comfy(backend: crate::startup::CanvasBackend) -> Result<Self, Box<dyn Error>> {
        Ok(Self { fixture: Fixture::comfy(backend)?, native: Compositor::default(), page_delay: 0 })
    }

    /// Address the original attachment `1` or a reopened one in the default workflow.
    pub(super) fn send(
        &mut self,
        attachment: u64,
        id: u64,
        kind: &str,
        extra: Value,
    ) -> Result<(), Box<dyn Error>> {
        self.send_to(Address { workflow: "workflow", attachment }, id, kind, extra)
    }

    /// Send one request through the decoded public envelope at an exact attachment address.
    pub(super) fn send_to(
        &mut self,
        address: Address<'_>,
        id: u64,
        kind: &str,
        mut extra: Value,
    ) -> Result<(), Box<dyn Error>> {
        extra
            .as_object_mut()
            .ok_or("Request fields absent")?
            .insert("address".into(), address.wire());
        self.fixture.advance(self.page_delay);
        self.fixture.send(id, kind, extra)
    }

    /// Hold the next selection of fixture `filename` in a gated acquisition.
    pub(super) fn gate(&mut self, filename: &str, cooperative: bool) -> Arc<Gate> {
        let gate = Gate::new(self.fixture.output(filename), cooperative, self.fixture.wake.clone());
        self.fixture.canvas.surface.gate_fixture(filename, gate.clone());
        gate
    }

    /// Advance the canvas without composition until `reached` holds; an ordinary reply first
    /// is returned as an error naming `stage`.
    pub(super) fn pump_until(
        &mut self,
        stage: &str,
        reached: impl Fn(&Fixture) -> bool,
    ) -> Result<(), Box<dyn Error>> {
        for _ in 0..4096 {
            let observed = self.fixture.wake.revision();
            self.fixture.pump()?;
            if reached(&self.fixture) {
                return Ok(());
            }
            if self.fixture.canvas.take_command().is_some() {
                return Err(format!("Replied before {stage}").into());
            }
            if self.fixture.wake.revision() != observed {
                continue;
            }
            // A completion held back for presentation cadence waits on the reported deadline.
            if self.fixture.execution()? != Execution::Running
                && self.fixture.canvas.next_deadline().is_some()
            {
                self.fixture.advance(100);
                continue;
            }
            self.fixture
                .wake
                .wait_after(observed)
                .map_err(|cause| format!("{stage} never arrived: {cause}"))?;
        }
        Err(format!("{stage} kept progressing without arriving").into())
    }

    /// Run one UI turn: drain completions, poll, then let native composition acknowledge.
    /// Returns a correlated reply, or whether this turn made observable progress.
    pub(super) fn turn(&mut self, id: u64) -> Result<Result<Value, bool>, Box<dyn Error>> {
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
    pub(super) fn expect(&mut self, id: u64, status: &str) -> Result<Value, Box<dyn Error>> {
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
    pub(super) fn quiesce(&mut self) -> Result<(), Box<dyn Error>> {
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
    pub(super) fn until_loaded(&mut self, id: u64) -> Result<bool, Box<dyn Error>> {
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
    pub(super) fn cancel(&mut self, id: u64) -> Result<Option<String>, Box<dyn Error>> {
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

    /// Shades of every frame the fake compositor committed, oldest first.
    pub(super) fn committed(&self) -> Vec<Shade> {
        self.native.committed.iter().map(|(_, shade)| *shade).collect()
    }

    /// Shade the native tree currently shows, if any.
    pub(super) fn shown(&self) -> Option<Shade> {
        self.native.active?;
        self.native.committed.last().map(|(_, shade)| *shade)
    }

    /// Retire the canvas scope and its execution owner, discarding any unread reply.
    pub(super) fn finish(mut self) -> Result<(), Box<dyn Error>> {
        while self.fixture.canvas.take_command().is_some() {}
        self.fixture.finish()
    }
}

/// Decode an ordinary reply and require it to answer request `id`.
pub(super) fn surface_reply(
    command: DesktopStartupCommand,
    id: u64,
) -> Result<Value, Box<dyn Error>> {
    let DesktopStartupCommand::SendSurface(reply) = command else {
        return Err("Expected an ordinary surface reply".into());
    };
    let value: Value = serde_json::from_slice(reply.payload())?;
    if value["requestId"] != id {
        return Err(format!("Expected reply {id}, received {value}").into());
    }
    Ok(value)
}

/// Shared CSS geometry at the fixture's origin; a hidden surface publishes an empty clip.
pub(super) fn geometry(width: u32, height: u32, visible: bool) -> Value {
    let clip = if visible { (width, height) } else { (0, 0) };
    json!({"geometry":{
        "coordinateSpace":"client", "units":"css-pixels", "origin":{"x":10,"y":20},
        "viewport":{"width":width,"height":height},
        "clip":{"x":10,"y":20,"width":clip.0,"height":clip.1},
        "physicalPixelsPerCssPixel":1, "visible":visible, "focused":false
    }})
}

/// Select one fixture output by filename.
pub(super) fn select(filename: &str) -> Value {
    json!({
        "itemId":format!("result:{filename}"),
        "file":{"filename":filename,"subfolder":"","type":"output"}
    })
}
