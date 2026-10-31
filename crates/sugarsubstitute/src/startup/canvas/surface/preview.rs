// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded preview transfer custody independently of ordinary camera request settlement.
mod assembly;
mod receive;
mod status;
use super::*;
use crate::native_canvas::session::{PreviewOffer, PreviewStatus, PreviewToken};
use assembly::Assembly;
use sugarsubstitute_contracts::{
    canvas_surface::{
        CanvasSurfaceDiscoveryStatus, CanvasSurfacePresentationChange,
        CanvasSurfacePresentationDisplay, CanvasSurfacePresentationRequest,
        CanvasSurfacePresentationStatus, CanvasViewportMode, CanvasViewportState,
    },
    execution::MemoryLease,
};
use sugarsubstitute_desktop::SurfacePresentation;

/// Resident admission for one watch or lease label, separate from transfer byte custody.
const PREVIEW_METADATA_BYTES: u64 = 32 * 1024;

/// A watch owns metadata separately from each emitted snapshot's callback lifetime.
struct Watch {
    identity: MainIdentity,
    memory: MemoryLease,
    _claim: MemoryClaim,
}
/// External lease labels retain their original admission through native token correlation.
struct Lease {
    identity: MainIdentity,
    address: CanvasSurfaceAddress,
    token: String,
    revision: u64,
    native: Option<PreviewToken>,
    _claim: MemoryClaim,
}
/// A complete or aborted transfer keeps its original until a storage worker accepts custody.
struct Stage {
    assembly: Assembly,
    lease: Option<Lease>,
    original: Option<ImageSource>,
    ended: bool,
    aborted: bool,
}
/// Display evidence changes only after an actual native frame acknowledgement.
struct Shown {
    lease: Lease,
    viewport: CanvasViewportState,
}

impl Shown {
    /// Describe these accepted native pixels as the observable display.
    fn display(&self) -> CanvasSurfacePresentationDisplay {
        CanvasSurfacePresentationDisplay::Preview {
            token: self.lease.token.clone(),
            lease_revision: self.lease.revision,
            viewport: self.viewport,
        }
    }
}
/// One assembly, one latest native candidate, one shown preview and one coalescible snapshot.
#[derive(Default)]
pub(super) struct Preview {
    watch: Option<Watch>,
    stage: Option<Stage>,
    current: Option<Lease>,
    withdrawing: bool,
    shown: Option<Shown>,
    sequence: u64,
    pending_change: Option<(MainIdentity, CanvasSurfacePresentationChange)>,
    pub(super) notification: Option<DesktopStartupCommand>,
}

impl Preview {
    /// Drop only observation metadata; staged allocations retain separate worker custody.
    pub(super) fn close_watch(&mut self) {
        self.watch = None;
        self.pending_change = None;
        self.notification = None;
    }

    /// Release small resident-backed labels before the session waits for complete scope custody.
    pub(super) fn close_correlations(&mut self) {
        self.current = None;
        self.shown = None;
        self.withdrawing = false;
    }
}

impl NativeSurface {
    /// Enable observation before byte admission; disabling fences existing native preview intent.
    pub(super) fn watch_presentation(
        &mut self,
        identity: MainIdentity,
        request: CanvasSurfacePresentationRequest,
        claim: MemoryClaim,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        if request.session() != &sugarsubstitute_contracts::panel::PanelSession::for_main(identity)?
        {
            return self.enqueue(DesktopStartupCommand::SendSurface(SurfaceReply::presentation(
                identity,
                &request,
                CanvasSurfaceDiscoveryStatus::Unavailable,
                false,
                "",
                claim,
            )?));
        }
        let available = self.available(identity) && session.initialized();
        if available
            && request.enabled()
            && let Some(previous) = self
                .preview
                .watch
                .as_ref()
                .map(|watch| watch.identity)
                .filter(|previous| *previous != identity)
        {
            self.retire_preview_navigation(previous, Some(session))?;
        }
        let status = if available {
            CanvasSurfaceDiscoveryStatus::Available
        } else {
            CanvasSurfaceDiscoveryStatus::Unavailable
        };
        if !request.enabled() {
            if self.preview.watch.as_ref().is_some_and(|watch| watch.identity == identity) {
                self.fence_previews(session)?;
                self.preview.watch = None;
                self.preview.pending_change = None;
                self.preview.notification = None;
            }
        } else if available
            && self.preview.watch.is_none()
            && let Some(memory) = session.preview_resident()
        {
            match memory.claim(PREVIEW_METADATA_BYTES) {
                Ok(metadata) => {
                    self.preview.watch = Some(Watch { identity, memory, _claim: metadata })
                }
                Err(cause) => {
                    tracing::warn!(operation="canvas_presentation_watch_admission", request_id=request.request_id(), cause=%cause, "Presentation watch admission is unavailable")
                }
            }
        }
        let enabled = self.preview.watch.as_ref().is_some_and(|watch| watch.identity == identity);
        self.enqueue(DesktopStartupCommand::SendSurface(SurfaceReply::presentation(
            identity, &request, status, enabled, "", claim,
        )?))
    }

    /// Cancel preview intent while preserving shown-image evidence until native replacement/hide.
    /// Intent that never reached native pixels settles at once: no frame or hide will follow it.
    pub(super) fn fence_previews(&mut self, session: &mut Session) -> Result<(), Box<dyn Error>> {
        let staged = self.preview.stage.as_mut().and_then(|stage| stage.lease.take());
        self.abort_preview_stage(session, CanvasSurfacePresentationStatus::Cancelled)?;
        session.withdraw_preview()?;
        if let Some(staged) = staged
            && let Some(previous) = self.preview.current.replace(staged)
        {
            self.preview_change(&previous, CanvasSurfacePresentationStatus::Cancelled)?;
        }
        if let Some(lease) = self.preview.current.take() {
            self.preview_change(&lease, CanvasSurfacePresentationStatus::Cancelled)?;
            if self.preview.shown.is_some() || session.preview_active() {
                self.preview.current = Some(lease);
                self.preview.withdrawing = true;
            }
        }
        Ok(())
    }
    /// Retire navigation-owned preview custody even when no canvas attachment was ever prepared.
    pub(super) fn retire_preview_navigation(
        &mut self,
        identity: MainIdentity,
        session: Option<&mut Session>,
    ) -> Result<(), Box<dyn Error>> {
        if self.preview.watch.as_ref().is_some_and(|watch| watch.identity == identity)
            || self.preview.current.as_ref().is_some_and(|lease| lease.identity == identity)
            || self.preview.shown.as_ref().is_some_and(|shown| shown.lease.identity == identity)
        {
            if let Some(session) = session {
                self.fence_previews(session)?;
            } else if self.preview.stage.is_some() {
                return Err("Preview allocation lost its retirement session".into());
            }
            self.preview.close_watch();
            self.preview.close_correlations();
        }
        Ok(())
    }

    /// Return raw or immutable originals through execution-owned disposal, retaining refusal.
    fn retire_preview_stage(&mut self, session: &mut Session) -> Result<bool, Box<dyn Error>> {
        let Some(stage) = &mut self.preview.stage else { return Ok(true) };
        if !stage.aborted {
            return Ok(false);
        }
        if !session.retire_preview_original(&mut stage.original)? {
            return Ok(false);
        }
        if !session.retire_preview_bytes(stage.assembly.retirement())? {
            return Ok(false);
        }
        self.preview.stage = None;
        Ok(true)
    }

    /// Mark a matching failed transfer once; its allocation remains under explicit retirement.
    fn abort_preview_stage(
        &mut self,
        session: &mut Session,
        status: CanvasSurfacePresentationStatus,
    ) -> Result<(), Box<dyn Error>> {
        let lease = self.preview.stage.as_mut().and_then(|stage| {
            stage.aborted = true;
            stage.lease.take()
        });
        if let Some(lease) = lease {
            self.preview_change(&lease, status)?;
        }
        self.retire_preview_stage(session)?;
        Ok(())
    }

    /// Observe original lease expiry, retry bounded worker admission and publish full snapshots.
    pub(super) fn poll_previews(&mut self, session: &mut Session) -> Result<(), Box<dyn Error>> {
        if self
            .preview
            .stage
            .as_ref()
            .is_some_and(|stage| !stage.aborted && session.preview_now() >= stage.assembly.deadline)
        {
            self.abort_preview_stage(session, CanvasSurfacePresentationStatus::Failed)?;
        }
        self.retire_preview_stage(session)?;
        let offer = match &mut self.preview.stage {
            Some(stage) if stage.ended && !stage.aborted => {
                session.offer_preview(&mut stage.original, stage.assembly.deadline)?
            }
            _ => PreviewOffer::Busy,
        };
        match offer {
            PreviewOffer::Offered(token) => {
                let stage = self.preview.stage.take().ok_or("Preview assembly disappeared")?;
                let mut lease = stage.lease.ok_or("Preview lease disappeared")?;
                lease.native = Some(token);
                if let Some(previous) = self.preview.current.replace(lease) {
                    self.preview_change(&previous, CanvasSurfacePresentationStatus::Cancelled)?;
                }
                session.request_host_graphics()?;
            }
            PreviewOffer::Expired => {
                self.abort_preview_stage(session, CanvasSurfacePresentationStatus::Failed)?;
            }
            PreviewOffer::Busy => {}
        }
        if !self.preview.withdrawing
            && let Some(lease) = &self.preview.current
        {
            let status =
                session.preview_status(lease.native.ok_or("Native preview identity disappeared")?);
            if matches!(status, PreviewStatus::Failed | PreviewStatus::Cancelled) {
                let lease = self.preview.current.take().ok_or("Preview lease disappeared")?;
                self.preview_change(
                    &lease,
                    if status == PreviewStatus::Failed {
                        CanvasSurfacePresentationStatus::Failed
                    } else {
                        CanvasSurfacePresentationStatus::Cancelled
                    },
                )?;
            }
        }
        self.queue_preview_notification()
    }

    /// Include transfer custody in shutdown settlement even before native work was admitted.
    pub(in crate::startup::canvas) fn preview_custody_settled(&self) -> bool {
        self.preview.stage.is_none()
    }

    /// Wake at the immutable begin deadline through the existing application event loop.
    pub(in crate::startup::canvas) fn preview_deadline(&self, now: Moment) -> Option<Instant> {
        let deadline =
            self.preview.stage.as_ref().filter(|stage| !stage.aborted)?.assembly.deadline;
        Instant::now()
            .checked_add(Duration::from_nanos(deadline.as_nanos().saturating_sub(now.as_nanos())))
    }
}
