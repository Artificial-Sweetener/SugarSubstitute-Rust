// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Acknowledge staged preview custody without occupying ordinary native presentation waiters.
use super::*;

impl NativeSurface {
    /// Qualify each request against the attachment while keeping lease revisions independent.
    pub(in crate::startup::canvas::surface) fn receive_preview(
        &mut self,
        identity: MainIdentity,
        request: Box<CanvasSurfaceRequest>,
        claim: MemoryClaim,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        let authorized = self.available(identity)
            && request.session()
                == &sugarsubstitute_contracts::panel::PanelSession::for_main(identity)?
            && self.preview.watch.as_ref().is_some_and(|watch| watch.identity == identity)
            && self.binding.as_ref().is_some_and(|binding| {
                binding.identity == identity
                    && binding.address == *request.address()
                    && binding.committed
                    && binding.placement.is_some()
                    && binding.document.is_none()
            })
            && !self.restoring
            && !self.pending.as_ref().is_some_and(|pending| {
                matches!(
                    pending.kind,
                    PendingKind::Select
                        | PendingKind::Hide
                        | PendingKind::EmptyCommit
                        | PendingKind::Prepare
                        | PendingKind::Restore
                )
            });
        if !authorized {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Unavailable, claim);
        }
        let binding = self.binding.as_mut().ok_or("Preview attachment disappeared")?;
        if request.revision() <= binding.revision {
            return self.reply(identity, &request, CanvasSurfaceReplyStatus::Stale, claim);
        }
        binding.revision = request.revision();
        let status = match request.operation() {
            CanvasSurfaceOperation::PreviewBegin(begin) => {
                if begin.format()
                    != sugarsubstitute_contracts::canvas_surface::CanvasSurfacePreviewFormat::Jpeg
                    || !session.presents_previews()
                    || self.preview.stage.is_some()
                    || self.preview.withdrawing
                {
                    CanvasSurfaceReplyStatus::Unavailable
                } else if let Some(memory) = session.preview_resident() {
                    match memory.claim(PREVIEW_METADATA_BYTES) {
                        Ok(metadata) => {
                            match Assembly::new(
                                begin.clone(),
                                request.revision(),
                                &memory,
                                session.preview_now(),
                            ) {
                                Ok(assembly) => {
                                    let lease = Lease {
                                        identity,
                                        address: request.address().clone(),
                                        token: begin.identity().token().into(),
                                        revision: request.revision(),
                                        native: None,
                                        _claim: metadata,
                                    };
                                    self.preview.stage = Some(Stage {
                                        assembly,
                                        lease: Some(lease),
                                        original: None,
                                        ended: false,
                                        aborted: false,
                                    });
                                    CanvasSurfaceReplyStatus::Applied
                                }
                                Err(cause) => {
                                    tracing::warn!(operation="canvas_preview_assembly_admission", address=?request.address(), request_id=request.request_id(), lease_revision=request.revision(), cause=%cause, "Preview assembly admission failed");
                                    CanvasSurfaceReplyStatus::Unavailable
                                }
                            }
                        }
                        Err(cause) => {
                            tracing::warn!(operation="canvas_preview_metadata_admission", address=?request.address(), request_id=request.request_id(), lease_revision=request.revision(), cause=%cause, "Preview metadata admission is unavailable");
                            CanvasSurfaceReplyStatus::Unavailable
                        }
                    }
                } else {
                    CanvasSurfaceReplyStatus::Unavailable
                }
            }
            CanvasSurfaceOperation::PreviewChunk(chunk) => {
                match self.preview.stage.as_mut().map(|stage| {
                    if !stage.assembly.owns(chunk.token(), chunk.lease_revision()) {
                        Err(assembly::Rejection::Foreign)
                    } else if stage.aborted || stage.ended {
                        Err(assembly::Rejection::Sequence)
                    } else {
                        stage.assembly.receive(chunk, session.preview_now())
                    }
                }) {
                    Some(Ok(())) => CanvasSurfaceReplyStatus::Applied,
                    Some(Err(assembly::Rejection::Foreign)) | None => {
                        CanvasSurfaceReplyStatus::Stale
                    }
                    Some(Err(_)) => {
                        self.abort_preview_stage(session, CanvasSurfacePresentationStatus::Failed)?;
                        CanvasSurfaceReplyStatus::Error
                    }
                }
            }
            CanvasSurfaceOperation::PreviewEnd(lease) => {
                let completed = self.preview.stage.as_ref().map(|stage| {
                    stage.assembly.complete(
                        lease.token(),
                        lease.lease_revision(),
                        session.preview_now(),
                    )
                });
                match completed {
                    Some(Ok(()))
                        if self
                            .preview
                            .stage
                            .as_ref()
                            .is_some_and(|stage| !stage.aborted && !stage.ended) =>
                    {
                        let memory =
                            session.preview_resident().ok_or("Preview admission disappeared")?;
                        let stage =
                            self.preview.stage.as_mut().ok_or("Preview assembly disappeared")?;
                        match stage.assembly.original(&memory) {
                            Ok(original) => {
                                stage.original = Some(original);
                                stage.ended = true;
                                CanvasSurfaceReplyStatus::Applied
                            }
                            Err(cause) => {
                                tracing::warn!(operation="canvas_preview_original_admission", address=?request.address(), request_id=request.request_id(), lease_revision=lease.lease_revision(), cause=%cause, "Preview original admission failed");
                                if matches!(
                                    cause.downcast_ref::<sugarsubstitute_image_io::ImportError>(),
                                    Some(sugarsubstitute_image_io::ImportError::Image(
                                        sugarsubstitute_image_ops::ImageError::Memory(_)
                                    ))
                                ) {
                                    CanvasSurfaceReplyStatus::Unavailable
                                } else {
                                    self.abort_preview_stage(
                                        session,
                                        CanvasSurfacePresentationStatus::Failed,
                                    )?;
                                    CanvasSurfaceReplyStatus::Error
                                }
                            }
                        }
                    }
                    Some(Err(assembly::Rejection::Foreign)) | None => {
                        CanvasSurfaceReplyStatus::Stale
                    }
                    _ => {
                        self.abort_preview_stage(session, CanvasSurfacePresentationStatus::Failed)?;
                        CanvasSurfaceReplyStatus::Error
                    }
                }
            }
            CanvasSurfaceOperation::PreviewWithdraw(lease) => {
                let owns = self.preview.stage.as_ref().is_some_and(|stage| {
                    stage.assembly.owns(lease.token(), lease.lease_revision())
                }) || self.preview.current.as_ref().is_some_and(|current| {
                    current.token == lease.token() && current.revision == lease.lease_revision()
                }) || self.preview.shown.as_ref().is_some_and(|shown| {
                    shown.lease.token == lease.token()
                        && shown.lease.revision == lease.lease_revision()
                });
                if owns {
                    self.fence_previews(session)?;
                    CanvasSurfaceReplyStatus::Applied
                } else {
                    CanvasSurfaceReplyStatus::Stale
                }
            }
            _ => return Err("Non-preview operation entered preview custody".into()),
        };
        self.reply(identity, &request, status, claim)?;
        self.poll_previews(session)
    }
}
