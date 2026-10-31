// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Apply admitted attachment geometry, final selection and commit operations.
use super::*;
#[cfg(test)]
use crate::native_canvas::session::SelectedOriginals;
use sugarsubstitute_canvas::NavigationCommand;
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
            CanvasSurfaceOperation::Overview(cells) => {
                let cells = cells
                    .into_iter()
                    .map(|cell| super::overview::OverviewCell { file: cell.file, rect: cell.rect })
                    .collect();
                self.overview(identity, request, claim, cells, session)
            }
            CanvasSurfaceOperation::Viewport(command) => {
                self.viewport(identity, request, claim, command, session)
            }
            CanvasSurfaceOperation::Geometry(geometry) => {
                let Some(allocation) = allocation else {
                    return self.reply(identity, &request, CanvasSurfaceReplyStatus::Error, claim);
                };
                let projected = match placement(&geometry, allocation).and_then(|projected| {
                    // Hidden geometry routes nothing, so only a shown viewport projects them.
                    let occluded = match projected {
                        Some(_) => occlusions(&geometry, allocation)?,
                        None => Vec::new(),
                    };
                    Ok((projected, occluded))
                }) {
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
                let (projected, occlusions) = projected;
                let preview_shown = self.has_shown_preview();
                let binding = self.binding.as_mut().ok_or("Canvas binding disappeared")?;
                binding.occlusions = occlusions;
                binding.modal = geometry.modal();
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
            CanvasSurfaceOperation::Select { file, comparison, inspection, .. } => {
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
                        overview: None,
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
                session.frame_selection(inspection.map(|inspection| NavigationCommand::Frame {
                    center: inspection.center(),
                    span: inspection.span(),
                }));
                self.await_selection(identity, request, claim, false, session)
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

impl NativeSurface {
    /// Hold an offered selection until its first matching native frame; an overview selection
    /// also locks the camera, while an ordinary selection releases that lock.
    pub(super) fn await_selection(
        &mut self,
        identity: MainIdentity,
        request: Box<CanvasSurfaceRequest>,
        claim: MemoryClaim,
        overview: bool,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        session.request_host_graphics()?;
        let binding = self.binding.as_mut().ok_or("Canvas binding disappeared")?;
        binding.selected = true;
        binding.overview = overview;
        binding.selection_revision = request.revision();
        binding.source_intent_revision = Some(session.source_intent_revision());
        self.pending = Some(Pending { identity, request, claim, kind: PendingKind::Select });
        Ok(())
    }
}

/// Admit only a Comfy output-storage reference that the backend adapter may fetch; the view
/// target owns the closed PNG/JPEG filename rule.
pub(super) fn admitted_output(file: &CanvasFileReference) -> Option<ComfyViewTarget> {
    if file.storage() != CanvasFileStorage::Output {
        return None;
    }
    ComfyViewTarget::new(file).ok()
}

/// Resolve a deterministic qualification fixture by leaf name inside its owned output folder.
#[cfg(test)]
pub(super) fn fixture_file(output: &Path, file: &CanvasFileReference) -> Option<PathBuf> {
    let mut components = Path::new(file.filename()).components();
    let leaf =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    (file.subfolder().is_empty() && leaf).then(|| output.join(file.filename()))
}
