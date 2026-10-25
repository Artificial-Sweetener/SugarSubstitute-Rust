// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish only matching document/source snapshots and retire superseded native source custody.
use super::*;
use sugarsubstitute_image_io::ImportError;

impl Session {
    /// Retain actual initialization results or transfer their complete custody to owned cleanup.
    pub(super) fn consume_initialization(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(outcome) = Self::take(&mut self.initializing) else {
            return Ok(());
        };
        match Self::terminal(outcome) {
            Ok((resources, scene)) if self.closing => self.retire(Retired {
                _resources: Some(resources),
                _scene: Some(scene),
                ..Default::default()
            })?,
            Ok((resources, scene)) => {
                self.resources = Some(resources);
                self.scene = Some(scene);
                if let Some(path) = self.options.image.take() {
                    self.open(path, CacheReuse::Compatible, None)?;
                } else {
                    self.picker = true;
                }
            }
            Err(cause) if self.closing => {
                tracing::debug!(operation="canvas_initialization_stopped", cause=%cause, "Canvas initialization reached a terminal outcome during close")
            }
            Err(cause) => self.fail(cause),
        }
        Ok(())
    }
    /// A cache rebuild changes source custody while preserving the document's authored history.
    pub(super) fn consume_load(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(completion) =
            self.loading.as_mut().and_then(|loading| loading.job.try_take_completion())
        else {
            return Ok(());
        };
        let loading = self.loading.take().ok_or("Canvas load identity was lost")?;
        let current = !self.closing && loading.intent == self.selection;
        match Self::terminal(completion.outcome) {
            Ok(scene) if !current => {
                self.retire(Retired { _scene: Some(scene), ..Default::default() })?
            }
            Ok(mut scene) => {
                if let Some(expected) = loading.expected {
                    if scene.source.as_ref().map(|source| source.descriptor().asset())
                        != Some(expected)
                    {
                        self.retire(Retired { _scene: Some(scene), ..Default::default() })?;
                        self.fail(WorkError::invalid(
                            "Original image changed; the preceding source cannot be reconstructed",
                        ));
                        return Ok(());
                    }
                    let current =
                        self.scene.as_mut().ok_or("Canvas rebuild has no current document")?;
                    std::mem::swap(&mut current.source, &mut scene.source);
                    current.source_revision = scene.source_revision;
                    self.retire(Retired { _scene: Some(scene), ..Default::default() })?;
                } else {
                    self.view.fit(scene.document.snapshot().extent())?;
                    if let Some(previous) = self.scene.replace(scene) {
                        self.retire(Retired { _scene: Some(previous), ..Default::default() })?;
                    }
                    self.rebuilt = false;
                }
                self.failed = None;
                self.title = self.labels.title.to_owned();
            }
            Err(cause) if !current => {
                tracing::debug!(operation="canvas_source_stopped", cause=%cause, "Superseded canvas source reached a terminal outcome")
            }
            Err(cause) => self.fail(cause),
        }
        Ok(())
    }
    /// A current accepted camera request may finish while later camera intent is coalesced.
    /// Its pixels are always labelled with their actual camera and current authored revision.
    pub(super) fn consume_frame(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(outcome) = Self::take(&mut self.rendering) else {
            return Ok(());
        };
        match Self::terminal(outcome) {
            Ok(mut packet) => {
                let current = !self.closing
                    && self.pending_open.is_none()
                    && self.loading.is_none()
                    && self.scene.as_ref().is_some_and(|scene| {
                        let snapshot = scene.document.snapshot();
                        packet.document == (snapshot.identity(), snapshot.revision())
                            && packet.source_revision == scene.source_revision
                    });
                if !current {
                    self.retire(Retired { _packet: Some(packet), ..Default::default() })?;
                    return Ok(());
                }
                if let Some(replacement) = packet.replacement.take()
                    && let Some(previous) = self.front.replace(replacement)
                {
                    self.retire(Retired { _front: Some(previous), ..Default::default() })?;
                }
                let work = self
                    .front
                    .as_mut()
                    .ok_or("Canvas retained viewport is unavailable")?
                    .apply(&packet.update)?;
                let base = self.presented.as_ref().and_then(|_| packet.update.base());
                self.presented = Some(Presented {
                    view: packet.view,
                    filter: packet.filter,
                    document: packet.document,
                    source_revision: packet.source_revision,
                    damage: *packet.update.plan(),
                    base,
                });
                self.last_prepare = packet.elapsed;
                self.last_stats = packet.stats;
                self.last_surface_work = work;
                if self.scene.as_ref().is_some_and(|scene| scene.source.is_some()) {
                    self.title = self.labels.viewing(packet.view.zoom());
                }
            }
            Err(cause) if self.closing || self.pending_open.is_some() => {
                tracing::debug!(operation="canvas_frame_stopped", cause=%cause, "Canvas frame reached a terminal outcome before replacement")
            }
            Err(cause) => {
                if !self.rebuilt
                    && corrupt(cause.as_ref())
                    && let Some(scene) = &self.scene
                    && let (Some(path), Some(source)) = (&scene.original, &scene.source)
                {
                    let path = path.clone();
                    let expected = source.descriptor().asset();
                    self.rebuilt = true;
                    self.open(path, CacheReuse::Rebuild, Some(expected))?;
                    return Ok(());
                }
                self.fail(cause);
            }
        }
        Ok(())
    }
}
/// Match the typed cache-integrity cause without interpreting error text as a recovery command.
fn corrupt(mut cause: &(dyn Error + 'static)) -> bool {
    for _ in 0..16 {
        if cause
            .downcast_ref::<ImportError>()
            .is_some_and(|cause| matches!(cause, ImportError::Corrupt(_)))
        {
            return true;
        }
        let Some(next) = cause.source() else {
            break;
        };
        cause = next;
    }
    false
}
