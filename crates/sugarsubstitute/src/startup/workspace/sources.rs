// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve native final sources from exact adopted output/runtime bindings before canvas dispatch.
use super::*;
use crate::startup::canvas::session_source::{OriginalSpec, SessionSource};
use sugarsubstitute_contracts::canvas_surface::{
    CanvasSurfaceMessage, CanvasSurfaceOperation, CanvasSurfaceRequest,
};
use sugarsubstitute_desktop::SurfaceObservation;

fn outputs(
    request: &CanvasSurfaceRequest,
) -> impl Iterator<Item = (&str, &sugarsubstitute_contracts::canvas_surface::CanvasFileReference)> {
    let (primary, compared, overview) = match request.operation() {
        CanvasSurfaceOperation::Select { item_id, file, comparison, .. } => (
            Some((item_id.as_str(), file)),
            comparison.as_ref().map(|compared| (compared.item_id.as_str(), &compared.file)),
            None,
        ),
        CanvasSurfaceOperation::Overview(cells) => (None, None, Some(cells.as_slice())),
        _ => (None, None, None),
    };
    primary
        .into_iter()
        .chain(compared)
        .chain(overview.into_iter().flatten().map(|cell| (cell.item_id.as_str(), &cell.file)))
}
impl WorkspaceWork {
    pub(super) fn hold_surface(
        &mut self,
        observed: SurfaceObservation,
    ) -> Result<Option<SurfaceObservation>, Box<dyn Error>> {
        if !self.waits_for_binding(observed.message()) {
            return Ok(Some(observed));
        }
        if self.deferred_surface.len() >= 8 {
            return Err("Workspace native source queue saturated".into());
        }
        self.deferred_surface.push_back(observed);
        Ok(None)
    }
    fn waits_for_binding(&self, message: &CanvasSurfaceMessage) -> bool {
        let (Some(driver), CanvasSurfaceMessage::Request(request)) = (&self.driver, message) else {
            return false;
        };
        outputs(request).any(|(output, file)| {
            driver.pending_native_original(output, request.address())
                && matches!(
                    driver.resolve_native_original(output, request.address(), file),
                    workspace_session::NativeOriginalResolution::Unavailable
                )
        })
    }
    pub(super) fn take_surface(&mut self) -> Option<SurfaceObservation> {
        if self
            .deferred_surface
            .front()
            .is_none_or(|observed| self.waits_for_binding(observed.message()))
        {
            return None;
        }
        self.deferred_surface.pop_front()
    }
    pub(in crate::startup) fn native_source(
        &self,
        message: &CanvasSurfaceMessage,
        backend: Option<crate::startup::CanvasBackend>,
    ) -> Result<Option<SessionSource>, Box<dyn Error>> {
        let CanvasSurfaceMessage::Request(request) = message else {
            return Ok(None);
        };
        self.native_request_source(request, backend)
    }
    /// Replay routes consume the same classifier as the UUID-bearing original Select.
    pub(super) fn native_request_source(
        &self,
        request: &CanvasSurfaceRequest,
        backend: Option<crate::startup::CanvasBackend>,
    ) -> Result<Option<SessionSource>, Box<dyn Error>> {
        let (Some(driver), Some(store)) = (&self.driver, &self.store) else {
            return Ok(None);
        };
        if outputs(request).next().is_none() {
            return Ok(None);
        }
        let memory = self
            .delivery
            .as_ref()
            .ok_or("Workspace source allocation absent")?
            .claim(4 * 1024 * 1024)?;
        use workspace_session::NativeOriginalResolution;
        let mut decisions = Vec::new();
        decisions.try_reserve_exact(outputs(request).count())?;
        for (output, file) in outputs(request) {
            decisions.push((
                output,
                file,
                driver.resolve_native_original(output, request.address(), file),
            ));
        }
        let mut originals = Vec::new();
        originals.try_reserve_exact(decisions.len())?;
        for (output, file, decision) in decisions {
            if matches!(decision, NativeOriginalResolution::Unavailable) {
                originals.push(OriginalSpec::Unavailable);
                continue;
            }
            let target = match sugarsubstitute_comfy::output_media::ComfyViewTarget::new(file) {
                Ok(target) => target,
                Err(cause) => {
                    tracing::debug!(operation="workspace_native_source",output_id=%output,cause=%cause,"Native original format unavailable");
                    originals.push(OriginalSpec::Unavailable);
                    continue;
                }
            };
            match decision {
                NativeOriginalResolution::Verified { binding, asset: Some(asset), .. } => {
                    tracing::debug!(operation="workspace_native_original_selected",output_id=%output,runtime_session=%binding.runtime_session,generation=binding.generation,attachment_generation=request.address().generation(),digest=%asset.digest,bytes=asset.bytes,"Exact retained original selected for native acquisition");
                    originals
                        .push(OriginalSpec::Retained { path: store.asset_path(&asset)?, asset });
                }
                NativeOriginalResolution::Verified { digest, bytes, .. } => {
                    originals.push(OriginalSpec::Backend {
                        backend: backend
                            .ok_or("Current backend unavailable for mixed selection")?,
                        target,
                        expected: Some(
                            sugarsubstitute_comfy::output_media::OutputArtifactIdentity::new(
                                &digest, bytes,
                            )?,
                        ),
                    })
                }
                NativeOriginalResolution::Live => originals.push(OriginalSpec::Backend {
                    backend: backend
                        .ok_or("Current backend unavailable for mixed live selection")?,
                    target,
                    expected: None,
                }),
                NativeOriginalResolution::Unavailable => originals.push(OriginalSpec::Unavailable),
            }
        }
        Ok(Some(SessionSource::new(originals, memory)))
    }
}

impl<B: StartupBackend> StartupHost<B> {
    /// Drain bounded native route work after current Ready/publication state has been observed.
    pub(in crate::startup) fn advance_native_output_replays(
        &mut self,
    ) -> Result<(), Box<dyn Error>> {
        for _ in 0..8 {
            let Some(replay) = self.canvas.as_ref().and_then(canvas::EmbeddedCanvas::output_replay)
            else {
                break;
            };
            let backend = self.child.canvas_backend(replay.identity);
            let source = self
                .workspace
                .as_ref()
                .map(|workspace| workspace.native_request_source(&replay.selection, backend))
                .transpose();
            let source = match source {
                Ok(source) => source.flatten(),
                Err(cause) => {
                    tracing::warn!(operation="workspace_native_replay_refused",identity=?replay.identity,cause=%cause,"Selected original unavailable on output route");
                    None
                }
            };
            self.canvas
                .as_mut()
                .ok_or("Native replay owner disappeared")?
                .resume_output_replay(replay, source)?;
        }
        Ok(())
    }
}
