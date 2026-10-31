// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Apply admitted attachment geometry, final selection and commit operations.
use super::*;
impl NativeSurface {
    /// Apply an already-qualified operation through its authoritative session owner.
    pub(super) fn apply_operation(
        &mut self,
        identity: MainIdentity,
        request: Box<CanvasSurfaceRequest>,
        allocation: Option<WebViewAllocation>,
        claim: MemoryClaim,
        operation: CanvasSurfaceOperation,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        match operation {
            CanvasSurfaceOperation::Prepare => Err("Canvas preparation was not admitted".into()),
            CanvasSurfaceOperation::Release => self.release(identity, request, claim, session),
            CanvasSurfaceOperation::Viewport(command) => {
                self.viewport(identity, request, claim, command, session)
            }
            CanvasSurfaceOperation::Geometry(geometry) => {
                let Some(allocation) = allocation else {
                    return self.reply(identity, &request, CanvasSurfaceReplyStatus::Error, claim);
                };
                let projected = match placement(&geometry, allocation) {
                    Ok(value) => value,
                    Err(_) => {
                        return self.reply(
                            identity,
                            &request,
                            CanvasSurfaceReplyStatus::Error,
                            claim,
                        );
                    }
                };
                let preview_shown = self.has_shown_preview();
                let binding = self.binding.as_mut().ok_or("Canvas binding disappeared")?;
                let viewport = match projected {
                    Some(projected) => Some(projected.viewport),
                    None if !binding.committed => match preparation_viewport(&geometry, allocation)
                    {
                        Ok(viewport) if viewport.width > 0 && viewport.height > 0 => Some(viewport),
                        Ok(_) => None,
                        Err(_) => {
                            return self.reply(
                                identity,
                                &request,
                                CanvasSurfaceReplyStatus::Error,
                                claim,
                            );
                        }
                    },
                    None => None,
                };
                binding.placement = projected;
                binding.allocation = Some(allocation);
                let committed = binding.committed;
                let wait = committed
                    && (binding.selected || preview_shown)
                    && projected.is_some()
                    && binding.presented.is_none_or(|previous| {
                        Some(previous.placement) != projected || previous.allocation != allocation
                    });
                if let Some(viewport) = viewport {
                    session.resize(Extent::new(viewport.width, viewport.height)?)?;
                    session.input(Input::Drawable(true))?;
                } else {
                    self.fence_previews(session)?;
                    session.input(Input::Drawable(false))?;
                }
                if committed && projected.is_none() {
                    self.pending =
                        Some(Pending { identity, request, claim, kind: PendingKind::Hide });
                    return Ok(());
                }
                if wait {
                    self.pending =
                        Some(Pending { identity, request, claim, kind: PendingKind::Geometry });
                    Ok(())
                } else {
                    self.reply(identity, &request, CanvasSurfaceReplyStatus::Applied, claim)
                }
            }
            CanvasSurfaceOperation::Select { file, .. } => {
                let target = match ComfyViewTarget::new(&file) {
                    Ok(target) => target,
                    Err(_) => {
                        return self.reply(
                            identity,
                            &request,
                            CanvasSurfaceReplyStatus::Error,
                            claim,
                        );
                    }
                };
                if file.storage() != CanvasFileStorage::Output
                    || !file.filename().to_ascii_lowercase().ends_with(".png")
                {
                    return self.reply(identity, &request, CanvasSurfaceReplyStatus::Error, claim);
                }
                self.fence_previews(session)?;
                #[cfg(test)]
                if let Some(output) = &self.fixture_output {
                    let mut components = Path::new(file.filename()).components();
                    let leaf = matches!(components.next(), Some(Component::Normal(_)))
                        && components.next().is_none();
                    if !file.subfolder().is_empty() || !leaf {
                        return self.reply(
                            identity,
                            &request,
                            CanvasSurfaceReplyStatus::Error,
                            claim,
                        );
                    }
                    let mut source = Some(ImageSource::file(output.join(file.filename())));
                    if !session.offer_source(&mut source, CacheReuse::Compatible, None)? {
                        return self.reply(
                            identity,
                            &request,
                            CanvasSurfaceReplyStatus::Unavailable,
                            claim,
                        );
                    }
                } else if !self.offer_comfy(identity, target, session)? {
                    return self.reply(
                        identity,
                        &request,
                        CanvasSurfaceReplyStatus::Unavailable,
                        claim,
                    );
                }
                #[cfg(not(test))]
                if !self.offer_comfy(identity, target, session)? {
                    return self.reply(
                        identity,
                        &request,
                        CanvasSurfaceReplyStatus::Unavailable,
                        claim,
                    );
                }
                session.request_host_graphics()?;
                self.binding.as_mut().ok_or("Canvas binding disappeared")?.selected = true;
                self.binding.as_mut().ok_or("Canvas binding disappeared")?.selection_revision =
                    request.revision();
                self.binding.as_mut().ok_or("Canvas binding disappeared")?.source_intent_revision =
                    Some(session.source_intent_revision());
                self.pending =
                    Some(Pending { identity, request, claim, kind: PendingKind::Select });
                Ok(())
            }
            CanvasSurfaceOperation::Commit => {
                let binding = self.binding.as_mut().ok_or("Canvas binding disappeared")?;
                if binding.placement.is_none() {
                    return self.reply(identity, &request, CanvasSurfaceReplyStatus::Error, claim);
                }
                binding.committed = true;
                let kind =
                    if binding.selected { PendingKind::Commit } else { PendingKind::EmptyCommit };
                self.pending = Some(Pending { identity, request, claim, kind });
                Ok(())
            }
            CanvasSurfaceOperation::PreviewBegin(_)
            | CanvasSurfaceOperation::PreviewChunk(_)
            | CanvasSurfaceOperation::PreviewEnd(_)
            | CanvasSurfaceOperation::PreviewWithdraw(_) => {
                Err("Preview custody escaped its dedicated owner".into())
            }
        }
    }
}
