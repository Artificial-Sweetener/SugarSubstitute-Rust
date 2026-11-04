// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Apply admitted attachment geometry, final selection and commit operations.
use super::*;
use crate::native_canvas::session::RetentionKey;
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
                // A selection deferred while hidden keeps the one pending slot; its own reply
                // proves presentation, so this geometry is answered as soon as it applies.
                let deferred = self
                    .pending
                    .as_ref()
                    .is_some_and(|pending| matches!(pending.kind, PendingKind::Select));
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
                    && (binding.shows_document(session) || preview_shown)
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
                if committed && projected.is_none() && !deferred {
                    self.hold(identity, request, claim, PendingKind::Hide);
                    return Ok(());
                }
                if wait && !deferred {
                    self.hold(identity, request, claim, PendingKind::Geometry);
                    Ok(())
                } else {
                    self.reply(identity, &request, CanvasSurfaceReplyStatus::Applied, claim)
                }
            }
            CanvasSurfaceOperation::Select { file, comparison, inspection, .. } => {
                let compared = comparison.as_ref().map(|comparison| &comparison.file);
                let drag_name = compared.is_none().then(|| output_leaf(&file));
                let Some(target) = admitted_output(&file) else {
                    return self.reply(identity, &request, CanvasSurfaceReplyStatus::Error, claim);
                };
                let Ok(compared_target) =
                    compared.map(|file| admitted_output(file).ok_or(())).transpose()
                else {
                    return self.reply(identity, &request, CanvasSurfaceReplyStatus::Error, claim);
                };
                self.fence_previews(session)?;
                // Only a single output is retained; comparisons are always acquired again.
                let retention = compared.is_none().then(|| self.retention_key(identity, &target));
                match retention.flatten() {
                    Some(key) if session.reselect_retained(&key)? => {
                        tracing::debug!(
                            operation = "canvas_selection_retained",
                            request_id = request.request_id(),
                            source_intent = session.source_intent_revision(),
                            "Reselected a retained canvas output without acquiring it"
                        );
                    }
                    retention => {
                        match self.offer_selection(
                            identity,
                            &file,
                            compared,
                            target,
                            compared_target,
                            session,
                        )? {
                            Some(true) => {}
                            Some(false) => {
                                return self.reply(
                                    identity,
                                    &request,
                                    CanvasSurfaceReplyStatus::Unavailable,
                                    claim,
                                );
                            }
                            None => {
                                return self.reply(
                                    identity,
                                    &request,
                                    CanvasSurfaceReplyStatus::Error,
                                    claim,
                                );
                            }
                        }
                        if let Some(key) = retention {
                            session.retain_selection(key);
                        }
                    }
                }
                self.workflow.offered(&file, compared);
                session.frame_selection(inspection.map(|inspection| NavigationCommand::Frame {
                    center: inspection.center(),
                    span: inspection.span(),
                }));
                self.await_selection(identity, request, claim, false, session)?;
                self.binding.as_mut().ok_or("Canvas binding disappeared")?.drag_name = drag_name;
                Ok(())
            }
            CanvasSurfaceOperation::Commit => {
                let binding = self.binding.as_mut().ok_or("Canvas binding disappeared")?;
                if binding.placement.is_none() {
                    return self.reply(identity, &request, CanvasSurfaceReplyStatus::Error, claim);
                }
                binding.committed = true;
                let kind = if binding.shows_document(session) {
                    PendingKind::Commit
                } else {
                    PendingKind::EmptyCommit
                };
                self.hold(identity, request, claim, kind);
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
    /// Offer the selected output, with any comparison output, as the session's next source.
    /// Returns `Some(false)` while the session cannot accept a source and `None` when a
    /// qualification fixture reference does not resolve inside its output folder.
    pub(super) fn offer_selection(
        &mut self,
        identity: MainIdentity,
        file: &CanvasFileReference,
        compared: Option<&CanvasFileReference>,
        target: ComfyViewTarget,
        compared_target: Option<ComfyViewTarget>,
        session: &mut Session,
    ) -> Result<Option<bool>, Box<dyn Error>> {
        #[cfg(test)]
        if let Some(output) = &self.fixture_output {
            let (Some(primary), Ok(compared)) = (
                fixture_file(output, file),
                compared.map(|file| fixture_file(output, file).ok_or(())).transpose(),
            ) else {
                return Ok(None);
            };
            let gated = self
                .fixture_acquirers
                .iter()
                .position(|(name, _)| compared.is_none() && name == file.filename());
            let offered = if let Some(index) = gated {
                let mut source = Some(self.fixture_acquirers.swap_remove(index).1);
                session.offer_acquirer(&mut source, CacheReuse::Compatible)?
            } else {
                let mut originals = Some(SelectedOriginals {
                    primary: ImageSource::file(primary),
                    compared: compared.map(ImageSource::file),
                    overview: None,
                });
                session.offer_originals(&mut originals, CacheReuse::Compatible, None)?
            };
            return Ok(Some(offered));
        }
        #[cfg(not(test))]
        let _ = (file, compared);
        self.offer_comfy(identity, target, compared_target, session).map(Some)
    }

    /// Identify a single output for retention by the backend that serves it and its output
    /// reference; qualification fixtures are identified by their reference alone.
    pub(super) fn retention_key(
        &self,
        identity: MainIdentity,
        target: &ComfyViewTarget,
    ) -> Option<RetentionKey> {
        #[cfg(test)]
        if self.fixture_output.is_some() {
            return Some(RetentionKey::new(&format!("fixture/{}", target.as_str())));
        }
        let origin = self.backend.filter(|backend| backend.identity == identity)?.origin;
        Some(RetentionKey::new(&format!(
            "{:?}:{}/{}",
            origin.host(),
            origin.port(),
            target.as_str()
        )))
    }

    /// Acquire the next selection of the fixture file `filename` through `source`, so a test
    /// controls when its worker-side acquisition returns.
    #[cfg(test)]
    pub(in crate::startup::canvas) fn gate_fixture(
        &mut self,
        filename: &str,
        source: Arc<dyn crate::native_canvas::session::acquisition::SourceAcquirer>,
    ) {
        self.fixture_acquirers.push((filename.to_owned(), source));
    }

    /// Admit geometry beside a selection deferred on its hidden committed attachment, so the
    /// page can show the attachment that the selection waits to present on.
    pub(super) fn admits_beside_deferred_selection(
        &self,
        identity: MainIdentity,
        request: &CanvasSurfaceRequest,
    ) -> bool {
        matches!(request.operation(), CanvasSurfaceOperation::Geometry(_))
            && self.pending.as_ref().is_some_and(|pending| {
                matches!(pending.kind, PendingKind::Select) && pending.identity == identity
            })
            && self.binding.as_ref().is_some_and(|binding| {
                binding.committed
                    && binding.placement.is_none()
                    && binding.identity == identity
                    && binding.address == *request.address()
            })
    }

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
        binding.drag_name = None;
        binding.selection_revision = request.revision();
        binding.source_intent_revision = Some(session.source_intent_revision());
        super::lifecycle::offered(&request, session.source_intent_revision());
        self.hold(identity, request, claim, PendingKind::Select);
        self.trace_selection_wait(session);
        Ok(())
    }
}

/// The stored output's own filename leaf, without any subfolder component.
fn output_leaf(file: &CanvasFileReference) -> String {
    file.filename().rsplit(['/', '\\']).next().unwrap_or_default().to_owned()
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
