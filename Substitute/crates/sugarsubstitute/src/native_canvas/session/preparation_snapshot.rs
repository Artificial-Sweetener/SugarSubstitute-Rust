// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe existing native preparation ownership without changing its scheduling or publication.
use super::*;
use crate::canvas_preparation_diagnostic::{NativePreparationSnapshot, Pacer, Phase, Work};
use sugarsubstitute_canvas_render::PresentationStage;

impl Session {
    /// Copy current guards, job statuses and evidence; never drain, cancel or submit work.
    pub(crate) fn native_preparation_snapshot(&self) -> NativePreparationSnapshot {
        let observe = |status: JobStatus| match status.phase {
            JobPhase::Queued => Work::Queued,
            JobPhase::Running => Work::Running,
            JobPhase::Terminal(_) => Work::Terminal,
        };
        let render = self.rendering.as_ref().map(|r| (r.job.spec().id.get(), r.job.status()));
        let display = self.displaying.as_ref().map(|d| (d.job.spec().id.get(), d.job.status()));
        let pacer = match self.pacing.readiness(self.navigation_time()) {
            Ok(PresentationReadiness::Ready) => Pacer::Ready,
            Ok(PresentationReadiness::WaitingUntil(_)) => Pacer::Waiting,
            Ok(PresentationReadiness::Busy { stage: PresentationStage::Preparing, .. }) => {
                Pacer::Preparing
            }
            Ok(PresentationReadiness::Busy {
                stage: PresentationStage::AwaitingNative, ..
            }) => Pacer::AwaitingNative,
            Err(_) => Pacer::Fault,
        };
        let retiring = self.pending_retirement.is_some()
            || self.deferred_retirement.is_some()
            || self.retirement.is_some();
        let initializing = self.initialization_pending
            || self.initializing.is_some()
            || self.resources.is_none()
            || !self.gpu_ready;
        let backing = [
            &self.document.fallback_presented,
            &self.document.navigation_presented,
            &self.document.presented,
        ]
        .into_iter()
        .flatten()
        .any(|shown| {
            self.document.shown_key() == Some(shown.document)
                && self
                    .document
                    .scene
                    .as_ref()
                    .is_some_and(|scene| shown.source_revision == scene.source_revision)
        });
        let projection = self.document.cpu_projection.is_some()
            || self.composition_pending.is_some()
            || self.composition_shown.is_some();
        let graphics_pending = self.host_graphics_pending || self.host_graphics_job.is_some();
        let evidence = self
            .document
            .pending_cpu_evidence
            .or(self.document.pending_gpu_evidence)
            .or(self.document.display_evidence);
        let evidence_current = evidence.is_some_and(|e| {
            self.document.shown_key() == Some(e.document)
                && e.source_revision == self.selection
                && e.view.surface() == self.document.tools.navigation().viewport().surface()
        });
        let material_matches =
            evidence.is_some_and(|e| e.backdrop_generation == self.backdrops.generation);
        let borrowed = self.rendering.as_ref().is_some_and(|r| {
            let front = match r.tier {
                BackingTier::Fallback => &self.document.fallback_front,
                BackingTier::Navigation => &self.document.navigation_front,
                BackingTier::Detail => &self.document.front,
            };
            front.as_ref().is_some_and(|front| Arc::strong_count(front) > 1)
        });
        let states = [
            self.drawable,
            self.resources.is_some(),
            self.draws_current_selection(),
            self.source_loading(),
            self.document_suspended,
            self.input_route_pending(),
            self.document_transaction.is_some(),
            retiring,
            self.acquisition_retirement.is_some(),
            self.gpu_ready,
            graphics_pending,
            self.document.cpu_pending.is_some(),
            self.gpu_presentation_pending(),
            backing,
            projection,
            evidence_current,
            material_matches,
            render.is_some_and(|(_, s)| s.cancellation_requested),
            display.is_some_and(|(_, s)| s.cancellation_requested),
            borrowed,
            self.failed.is_some(),
            self.picker,
            self.preview_pending(),
            self.input_shown(),
            !self.backdrops.regions.is_empty(),
            self.rendering.as_ref().is_some_and(|r| r.superseded),
            self.document.display_buffer_extent.is_some(),
            self.document.display_buffer_extent
                == Some(self.document.tools.navigation().viewport().surface()),
        ];
        let flags = states
            .into_iter()
            .enumerate()
            .fold(0, |bits, (i, value)| bits | (u32::from(value) << i));
        let buffers =
            self.document.display_buffers.iter().filter(|buffer| buffer.is_some()).count() as u8;
        let phase = if self.closing {
            Phase::Closing
        } else if self.failed.is_some() {
            Phase::Failed
        } else if retiring || self.acquisition_retirement.is_some() {
            Phase::Retiring
        } else if render.is_some() {
            Phase::Rendering
        } else if display.is_some() {
            Phase::Displaying
        } else if initializing {
            Phase::Initializing
        } else if self.input_route_pending() {
            Phase::Routing
        } else if self.document_suspended || self.document_transaction.is_some() {
            Phase::Suspended
        } else if !self.drawable {
            Phase::Hidden
        } else if graphics_pending {
            Phase::Graphics
        } else if pacer != Pacer::Ready {
            Phase::Pacer
        } else if !self.draws_current_selection() {
            Phase::Source
        } else if !self.gpu_document() && backing && buffers == 0 {
            Phase::DisplayBuffers
        } else if projection && (!evidence_current || !material_matches) {
            Phase::StaleEvidence
        } else if backing {
            Phase::DisplayAdmission
        } else {
            Phase::RenderAdmission
        };
        NativePreparationSnapshot {
            phase,
            flags,
            scene_source: self.document.scene.as_ref().map_or(0, |scene| scene.source_revision),
            buffers,
            render_id: render.map_or(0, |(id, _)| id),
            render: render.map_or(Work::None, |(_, s)| observe(s)),
            display_id: display.map_or(0, |(id, _)| id),
            display: display.map_or(Work::None, |(_, s)| observe(s)),
            pacer,
        }
    }
}
