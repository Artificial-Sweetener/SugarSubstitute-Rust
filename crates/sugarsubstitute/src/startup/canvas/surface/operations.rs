// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Apply admitted attachment geometry, final selection and commit operations.
use super::*;
#[cfg(test)]
use crate::native_canvas::session::SelectedOriginals;
use sugarsubstitute_contracts::canvas_surface::CanvasFileReference;

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
            CanvasSurfaceOperation::Select { file, comparison, .. } => {
                let compared = comparison.as_ref().map(|comparison| &comparison.file);
                let Some(target) = admitted_output(&file) else {
                    return self.reply(identity, &request, CanvasSurfaceReplyStatus::Error, claim);
                };
                let Ok(compared_target) =
                    compared.map(|file| admitted_output(file).ok_or(())).transpose()
                else {
                    return self.reply(identity, &request, CanvasSurfaceReplyStatus::Error, claim);
                };
                self.fence_previews(session)?;
                #[cfg(test)]
                if let Some(output) = &self.fixture_output {
                    let (Some(primary), Ok(compared)) = (
                        fixture_file(output, &file),
                        compared.map(|file| fixture_file(output, file).ok_or(())).transpose(),
                    ) else {
                        return self.reply(
                            identity,
                            &request,
                            CanvasSurfaceReplyStatus::Error,
                            claim,
                        );
                    };
                    let mut originals = Some(SelectedOriginals {
                        primary: ImageSource::file(primary),
                        compared: compared.map(ImageSource::file),
                    });
                    if !session.offer_originals(&mut originals, CacheReuse::Compatible, None)? {
                        return self.reply(
                            identity,
                            &request,
                            CanvasSurfaceReplyStatus::Unavailable,
                            claim,
                        );
                    }
                } else if !self.offer_comfy(identity, target, compared_target, session)? {
                    return self.reply(
                        identity,
                        &request,
                        CanvasSurfaceReplyStatus::Unavailable,
                        claim,
                    );
                }
                #[cfg(not(test))]
                if !self.offer_comfy(identity, target, compared_target, session)? {
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

/// Admit only a PNG Comfy output reference that the backend adapter may fetch.
fn admitted_output(file: &CanvasFileReference) -> Option<ComfyViewTarget> {
    if file.storage() != CanvasFileStorage::Output
        || !file.filename().to_ascii_lowercase().ends_with(".png")
    {
        return None;
    }
    ComfyViewTarget::new(file).ok()
}

/// Resolve a deterministic qualification fixture by leaf name inside its owned output folder.
#[cfg(test)]
fn fixture_file(output: &Path, file: &CanvasFileReference) -> Option<PathBuf> {
    let mut components = Path::new(file.filename()).components();
    let leaf =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    (file.subfolder().is_empty() && leaf).then(|| output.join(file.filename()))
}
