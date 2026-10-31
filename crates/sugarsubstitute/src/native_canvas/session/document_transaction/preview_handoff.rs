// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Park the accepted final only at exact native preview acceptance and restore its camera intact.

use super::{preview::*, *};
use sugarsubstitute_desktop::canvas::CpuFrame;

impl Session {
    /// Borrow the unpublished candidate only after it owns the pacer's exact native slot.
    pub(in crate::native_canvas::session) fn preview_cpu_frame(
        &self,
    ) -> Option<(CpuFrame<'_>, &MemoryLease)> {
        let candidate = self.preview.candidate.as_ref()?;
        if candidate.cancelled
            || self.closing
            || !self.drawable
            || candidate.owner_selection != self.selection
            || self.clock.now() >= candidate.deadline
        {
            return None;
        }
        let Stage::Offered(document, resources) = &candidate.stage else {
            return None;
        };
        Some((document.cpu_frame()?, &resources.as_ref()?.memory))
    }

    /// Reidentify prepared pixels only when the ordinary display slot becomes idle.
    pub(super) fn offer_ready_preview(
        &mut self,
        candidate: &mut Candidate,
    ) -> Result<(), Box<dyn Error>> {
        if !self.preview_can_offer(candidate)
            || self.pacing.readiness(self.navigation_time())? != PresentationReadiness::Ready
        {
            return Ok(());
        }
        let Stage::Ready(ready) = &mut candidate.stage else {
            return Ok(());
        };
        let retained = ready.as_mut().ok_or("Preview candidate disappeared")?;
        let now = self.navigation_time();
        let next = self
            .frame_revision
            .checked_add(1)
            .ok_or("Canvas display revision exhausted")?
            .max(retained.drawing.target.generation.get());
        let target = FrameStamp {
            surface: retained.drawing.target.surface,
            generation: Generation::new(next)?,
        };
        let mut document = Box::new(document::DocumentState::new(retained.drawing.view));
        document.tools.set_content(
            Some(retained.scene.document.snapshot().extent()),
            Some(&retained.scene.document),
            now,
        )?;
        document.tools.freeze_at(retained.drawing.view, Some(&retained.scene.document), now)?;
        document.tools.set_policy(true, false, Some(&retained.scene.document), now)?;
        let work::DrawPixels::Cpu(projection) = &mut retained.drawing.pixels else {
            return Err("Preview requires CPU pixels".into());
        };
        if projection.target() != target {
            projection.reidentify_in_place(target)?;
        }
        retained.drawing.target = target;
        self.pacing.submitted(target, PresentationMode::Projected, now, now)?;
        self.pacing.prepared(target, now)?;
        let ready = ready.take().ok_or("Preview candidate disappeared")?;
        let work::preview::ReadyPreview {
            scene,
            resources,
            backing,
            spare_buffer,
            drawing,
            reconstruction,
            sampling_scale,
            elapsed,
            stats,
            source_io: _source_io,
        } = *ready;
        let work::DrawPixels::Cpu(projection) = drawing.pixels else {
            return Err("Preview requires CPU pixels".into());
        };
        document.scene = Some(scene);
        document.fallback_front = Some(backing);
        document.fallback_presented = Some(Presented {
            sampling_scale,
            tier: BackingTier::Fallback,
            coverage: drawing.coverage,
            view: drawing.coverage.viewport(),
            reconstruction,
            document: drawing.document,
            source_revision: drawing.source_revision,
            comparison: None,
        });
        document.cpu_projection = Some(projection);
        document.projection_damage = Some(DamagePlan::full(drawing.view.surface()));
        document.display_buffers = [Some(spare_buffer), None];
        document.display_buffer_extent = Some(drawing.view.surface());
        document.cpu_pending = Some(target);
        document.pending_cpu_evidence = Some(DisplayEvidence {
            underlay: drawing.underlay,
            tier: drawing.tier,
            target,
            source: drawing.source,
            view: drawing.view,
            coverage: drawing.coverage,
            projection: drawing.projection,
            document: drawing.document,
            source_revision: drawing.source_revision,
        });
        document.native_verified = drawing.verified;
        self.frame_revision = next;
        self.last_prepare = elapsed;
        self.last_stats = stats;
        candidate.stage = Stage::Offered(document, Some(resources));
        Ok(())
    }

    /// Accept only the exact offered target, preserving the latest actual final camera at this instant.
    /// Validate an already uploaded preview independently from new-borrow eligibility deadlines.
    pub(crate) fn preview_frame_acknowledgeable(&self, target: FrameStamp) -> bool {
        self.preview.candidate.as_ref().is_some_and(|candidate| {
            !candidate.cancelled && candidate.owner_selection == self.selection
                && matches!(&candidate.stage, Stage::Offered(document, _) if document.cpu_pending == Some(target)
                    && document.pending_cpu_evidence.is_some_and(|evidence| evidence.target == target))
        })
    }

    /// Accept only the exact offered target, preserving the latest actual final camera at this instant.
    pub(in crate::native_canvas::session) fn accept_preview_cpu(
        &mut self,
        target: FrameStamp,
    ) -> Result<bool, Box<dyn Error>> {
        let offered = self.preview_frame_acknowledgeable(target);
        if !offered {
            return Ok(false);
        }
        let evidence = match &self
            .preview
            .candidate
            .as_ref()
            .ok_or("Preview acceptance lost its candidate")?
            .stage
        {
            Stage::Offered(document, _) => document
                .pending_cpu_evidence
                .filter(|evidence| evidence.target == target)
                .ok_or("Preview native evidence disappeared")?,
            _ => return Err("Preview acceptance lost its pixels".into()),
        };
        let now = self.navigation_time();
        if self.preview.parked.is_none()
            && let Some(evidence) = self.document.display_evidence
        {
            self.document.tools.freeze_at(
                evidence.view,
                self.document.scene.as_ref().map(|scene| &scene.document),
                now,
            )?;
        }
        self.pacing.native_accepted(target, now)?;
        let candidate =
            self.preview.candidate.take().ok_or("Preview acceptance lost its candidate")?;
        let Stage::Offered(mut document, resources) = candidate.stage else {
            return Err("Preview acceptance lost its pixels".into());
        };
        document.pending_cpu_evidence = None;
        document.cpu_pending = None;
        document.native_presented = Some(target);
        let previous = std::mem::replace(&mut self.document, document);
        let retired_document = if self.preview.parked.is_none() {
            self.preview.parked = Some(RetainedDocument {
                document: previous,
                selection: self.selection,
                title: self.title.clone(),
            });
            None
        } else {
            Some(previous)
        };
        self.selection = evidence.source_revision;
        self.preview.shown = Some((candidate.token, target));
        self.preview.resize_deadline = None;
        self.preview.resize_extent = None;
        self.preview.last = Some((candidate.token, PreviewStatus::Presented(target)));
        if let Some(rendering) = &mut self.rendering {
            rendering.superseded = true;
            rendering.job.cancel();
        }
        let old_resources = std::mem::replace(&mut self.preview.resources, resources);
        self.preview.retired = Some(PreviewRetired {
            _document: retired_document,
            _resources: old_resources,
            _custody: candidate.custody,
            ..Default::default()
        });
        self.advance_preview_retirement()?;
        self.accept_display(evidence, true)?;
        Ok(true)
    }

    /// Restore parked final custody only after foreign jobs and disposal slots have settled.
    pub(in crate::native_canvas::session) fn advance_preview_restoration(
        &mut self,
    ) -> Result<(), Box<dyn Error>> {
        if !self.preview_can_restore() {
            return Ok(());
        }
        let now = self.navigation_time();
        let replacing = self.preview_replacing_final();
        self.abandon_cpu_frame(now)?;
        if !self.closing
            && !self.document_suspended
            && !replacing
            && self.drawable
            && self.pacing.readiness(now)? != PresentationReadiness::Ready
        {
            return Ok(());
        }
        let Some(previous) = self.preview.parked.as_mut() else {
            self.preview.restoring = false;
            return Ok(());
        };
        let extent = self.document.tools.navigation().viewport().surface();
        previous.document.tools.resize(
            extent,
            previous.document.scene.as_ref().map(|scene| &scene.document),
            now,
        )?;
        previous.document.projection_base = None;
        previous.document.projection_damage = Some(DamagePlan::full(extent));
        if !self.closing
            && !self.document_suspended
            && !replacing
            && self.drawable
            && self.pacing.readiness(now)? == PresentationReadiness::Ready
            && previous.document.display_evidence.is_some()
            && previous
                .document
                .cpu_projection
                .as_ref()
                .is_some_and(|projection| projection.view().target().surface() == extent)
        {
            let next =
                self.frame_revision.checked_add(1).ok_or("Canvas display revision exhausted")?;
            let old = previous
                .document
                .cpu_projection
                .as_mut()
                .ok_or("Restored projection disappeared")?;
            let target =
                FrameStamp { surface: old.target().surface, generation: Generation::new(next)? };
            old.reidentify_in_place(target)?;
            if let Some(mut evidence) = previous.document.display_evidence {
                evidence.target = target;
                self.pacing.submitted(target, PresentationMode::Projected, now, now)?;
                self.pacing.prepared(target, now)?;
                previous.document.cpu_pending = Some(target);
                previous.document.pending_cpu_evidence = Some(evidence);
            }
            self.frame_revision = next;
        } else if !self.closing {
            previous.document.display_evidence = None;
            previous.document.native_presented = None;
        }
        let previous = self.preview.parked.take().ok_or("Parked document disappeared")?;
        let preview = std::mem::replace(&mut self.document, previous.document);
        if !replacing {
            self.selection = previous.selection;
            self.title = previous.title;
        }
        self.preview.shown = None;
        self.preview.restoring = false;
        self.preview.resize_deadline = None;
        self.preview.resize_extent = None;
        let resources = self.preview.resources.take();
        self.retire(Retired {
            _document: Some(preview),
            _preview: Some(PreviewRetired { _resources: resources, ..Default::default() }),
            ..Default::default()
        })
    }
}
