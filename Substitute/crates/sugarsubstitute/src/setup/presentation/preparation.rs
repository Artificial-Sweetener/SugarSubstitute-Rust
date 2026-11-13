// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move immutable installer pages through the execution owner's cooperative CPU lane.
use super::*;
use crate::render_transfer::PreparationTransfer;
use sugarsubstitute_contracts::{
    execution::{CpuStep, CpuTask, JobContext},
    presentation::FrameIdentity,
};

/// Two inputs allow one active page and one replacement without allocating before admission.
pub(super) const INPUT_BYTES: u64 = 2 * SNAPSHOT_BYTES;

/// Exclusive font resources follow actual preparation completion, including ordinary failures.
/// The input lease funds snapshots independently of retained frame pixels. Native font-library
/// allocations are opaque; all Rust scene, scratch and raster allocations have separate claims.
pub(super) struct Renderer {
    resources: Option<SceneResources>,
    locale: Option<&'static str>,
    pub(super) inputs: MemoryLease,
}

/// Return fonts without manufacturing a frame on initialization or after a rendering error.
pub(super) struct Output {
    pub(super) renderer: Renderer,
    pub(super) frame: Option<io::Result<Prepared>>,
}

/// Initialize only admission custody; first font discovery happens during the first render job.
pub(super) struct Initialize(Option<MemoryLease>);
impl Initialize {
    /// Called only by an admitted CPU factory.
    pub(super) fn new(lease: MemoryLease) -> Self {
        Self(Some(lease))
    }
}
impl CpuTask<Output, io::Error> for Initialize {
    fn step(&mut self, _: &JobContext) -> io::Result<CpuStep<Output>> {
        let inputs = self.0.take().ok_or_else(invalid_state)?;
        Ok(CpuStep::Complete(Output {
            renderer: Renderer { resources: None, locale: None, inputs },
            frame: None,
        }))
    }
}

/// Retain bounded raster work between cooperative steps. The execution adapter owns scheduling,
/// cancellation and actual destruction. No native window is accessed by this task.
pub(super) struct Render {
    painting: Option<Painting>,
    snapshot: Option<Snapshot>,
    renderer: Option<Renderer>,
    identity: FrameIdentity,
    lease: Option<MemoryLease>,
    failure: Option<io::Error>,
}
impl Render {
    /// Take exclusive resources only inside the admitted worker factory. Rejected factories leave
    /// the original transfer intact so the UI can retry without destroying or rebuilding fonts.
    pub(super) fn new(
        renderer: PreparationTransfer<Renderer>,
        snapshot: PreparationTransfer<Snapshot>,
        identity: FrameIdentity,
        lease: MemoryLease,
    ) -> Self {
        let result = renderer.take().map_err(io::Error::other);
        let (renderer, failure) = match result {
            Ok(renderer) => (Some(renderer), None),
            Err(error) => (None, Some(error)),
        };
        let result = snapshot.take().map_err(io::Error::other);
        let (snapshot, input_failure) = match result {
            Ok(snapshot) => (Some(snapshot), None),
            Err(error) => (None, Some(error)),
        };
        Self {
            painting: None,
            snapshot,
            renderer,
            identity,
            lease: Some(lease),
            failure: failure.or(input_failure),
        }
    }

    /// Compose once, then yield between shared rendering/validation slices.
    fn advance(&mut self) -> io::Result<Option<Prepared>> {
        let renderer = self.renderer.as_mut().ok_or_else(invalid_state)?;
        if let Some(snapshot) = self.snapshot.take() {
            if renderer.locale != Some(snapshot.locale) {
                // Release the previous locale before discovering its replacement on this worker.
                renderer.resources = None;
                renderer.locale = None;
                renderer.resources = Some(SceneResources::new(snapshot.locale)?);
                renderer.locale = Some(snapshot.locale);
            }
            let resources = renderer.resources.as_mut().ok_or_else(invalid_state)?;
            let lease = self.lease.take().ok_or_else(invalid_state)?;
            let scene = compose(&snapshot, resources, Some(&lease))?;
            self.painting = Some(scene.into_paint(self.identity, lease)?);
            return Ok(None);
        }
        let resources = renderer.resources.as_mut().ok_or_else(invalid_state)?;
        let painting = self.painting.as_mut().ok_or_else(invalid_state)?;
        if !painting.step(resources)? {
            return Ok(None);
        }
        self.painting.take().ok_or_else(invalid_state)?.finish().map(Some)
    }

    /// Dispose failed scratch on the worker before returning the still-usable cache.
    fn complete(&mut self, frame: io::Result<Prepared>) -> io::Result<CpuStep<Output>> {
        self.painting = None;
        self.snapshot = None;
        self.lease = None;
        Ok(CpuStep::Complete(Output {
            renderer: self.renderer.take().ok_or_else(invalid_state)?,
            frame: Some(frame),
        }))
    }
}
impl CpuTask<Output, io::Error> for Render {
    fn step(&mut self, _: &JobContext) -> io::Result<CpuStep<Output>> {
        if let Some(error) = self.failure.take() {
            return Err(error);
        }
        match self.advance() {
            Ok(None) => Ok(CpuStep::Yield),
            Ok(Some(frame)) => self.complete(Ok(frame)),
            Err(error) => self.complete(Err(error)),
        }
    }
}

/// Report a violated private task protocol without panicking inside an execution worker.
fn invalid_state() -> io::Error {
    io::Error::other("Invalid installer preparation task state")
}
