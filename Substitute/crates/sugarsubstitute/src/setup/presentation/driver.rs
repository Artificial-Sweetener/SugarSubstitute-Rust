// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe installer preparation without blocking native input or publishing superseded pages.
use super::{preparation::*, *};
use crate::{job_sequence::JobSequence, render_transfer::PreparationTransfer};
use std::error::Error;
use sugarsubstitute_application::presentation::{FramePublicationDecision, FramePublicationGate};
use sugarsubstitute_contracts::{
    execution::{
        AdmissionError, CompletionBatch, ExecutionPort, Generation, JobClass, JobSpec, MemoryLease,
        Operation, OwnedJob, ReplacementKey, Scope, SurfaceId,
    },
    presentation::{FrameIdentity, FrameShape},
};
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_ui::scene::FramePaint;

mod delivery;
#[cfg(test)]
mod tests;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

/// Retain actual request ownership until the admitted factory's terminal outcome is observed.
struct Pending {
    input: PreparationTransfer<Snapshot>,
    identity: FrameIdentity,
    bytes: u64,
}
/// Original admitted operation retained separately from the worker's returned frame identity.
struct Active {
    job: OwnedJob<Output, io::Error>,
    request: Option<Pending>,
}

/// One exclusive renderer, one active job and one latest pending page for an installer surface.
/// The composition root owns execution, completion routing and shutdown. Every semantic UI edit
/// must call `invalidate` before routing results; resize and visibility invalidate automatically.
/// Native HWND operations and frame copies remain with the desktop adapter.
pub struct Driver {
    client: ExecutionClient,
    scope: Scope,
    jobs: JobSequence,
    gate: FramePublicationGate,
    next_sequence: Generation,
    renderer: Option<PreparationTransfer<Renderer>>,
    inputs: Option<MemoryLease>,
    pending: Option<Pending>,
    active: Option<Active>,
    dirty: bool,
    closing: bool,
}

impl Driver {
    /// Bind a dedicated child scope of the application's execution lifetime and its sole job-ID
    /// sequence. Construction does not create workers, discover fonts or allocate pixels.
    /// # Errors
    /// Preserves identity construction failures.
    pub fn new(
        client: ExecutionClient,
        scope: Scope,
        jobs: JobSequence,
        surface: SurfaceId,
        shape: FrameShape,
    ) -> Result<Self> {
        Ok(Self {
            client,
            scope,
            jobs,
            gate: FramePublicationGate::new(surface, shape, Generation::new(1)?, true),
            next_sequence: Generation::new(1)?,
            renderer: None,
            inputs: None,
            pending: None,
            active: None,
            dirty: true,
            closing: false,
        })
    }

    /// Invalidate before routing completion after edits, page navigation or interaction changes.
    /// Active preparation may finish and return fonts, but its old pixels cannot be published.
    /// # Errors
    /// Preserves generation exhaustion before changing the pending request.
    pub fn invalidate(&mut self) -> Result {
        self.gate.invalidate_state()?;
        self.pending = None;
        self.dirty = true;
        Ok(())
    }

    /// Stop preparing hidden surfaces and require a fresh snapshot when shown again.
    /// # Errors
    /// Preserves generation exhaustion without changing visibility.
    pub fn set_visible(&mut self, visible: bool) -> Result {
        let visible = visible && !self.closing;
        if self.gate.visible() != visible {
            self.gate.set_visible(visible)?;
            self.pending = None;
            self.dirty = true;
        }
        Ok(())
    }

    /// Route the root's completion batch and capture current display state under existing memory
    /// admission. Empty batches also retry saturated admission; this never drains another owner's
    /// mailbox or waits for workers. Call after invalidating any changed semantic state.
    /// # Errors
    /// Preserves invalid geometry, input-admission, preparation and actual execution failures.
    pub fn advance(
        &mut self,
        flow: &SetupFlow,
        view: View,
        batch: &CompletionBatch,
    ) -> Result<Option<Prepared>> {
        if !self.closing && self.gate.visible() {
            let shape = FrameShape::new(
                view.surface.width,
                view.surface.height,
                view.surface.scale.into(),
            )?;
            if self.gate.shape() != shape {
                self.gate.resize(shape)?;
                self.pending = None;
                self.dirty = true;
            }
        }
        let matching = self.active.as_ref().is_some_and(|active| {
            batch.notifications.iter().any(|notice| {
                notice.job_id == active.job.spec().id && notice.scope_id == self.scope.id()
            })
        });
        // A root failure can consume its completion batch before reaching this adapter. Closing
        // still inspects only this exact retained job, so missed wake metadata cannot strand it.
        let prepared = if matching || self.closing { self.take_terminal()? } else { None };
        if self.closing || !self.gate.visible() {
            return Ok(prepared);
        }
        if self.dirty
            && let Some(inputs) = &self.inputs
        {
            self.pending = None;
            let snapshot = Snapshot::capture(flow, view, inputs)?;
            let sequence = self.next_sequence;
            self.next_sequence = sequence.next()?;
            self.pending = Some(Pending {
                input: PreparationTransfer::new(snapshot),
                identity: FrameIdentity {
                    surface: self.gate.surface(),
                    state_generation: self.gate.state_generation(),
                    sequence,
                },
                bytes: METADATA_BYTES
                    + FramePaint::reservation_bytes(self.gate.shape())
                    + sugarsubstitute_desktop::installer::native::storage_bytes(
                        self.gate.shape(),
                        super::Action::ALL
                            .iter()
                            .filter(|action| action.text_slot().is_some())
                            .count(),
                    )?,
            });
            self.dirty = false;
        }
        self.submit()?;
        Ok(prepared)
    }

    /// Admission is atomic; rejected factories cannot take fonts or the pending page.
    fn submit(&mut self) -> Result {
        if self.active.is_some() || (!self.dirty && self.pending.is_none()) {
            return Ok(());
        }
        let initializing = self.renderer.is_none();
        let bytes = if initializing {
            INPUT_BYTES
        } else {
            self.pending.as_ref().ok_or("Installer render request absent")?.bytes
        };
        let revision = self.next_sequence;
        self.next_sequence = revision.next()?;
        let spec = JobSpec {
            id: self.jobs.allocate()?,
            operation: Operation::FramePrepare,
            class: JobClass::CpuInteractive,
            deadline: None,
            reservation_bytes: bytes,
            replacement: Some(ReplacementKey { surface: self.gate.surface(), revision }),
        };
        let result = if initializing {
            self.client
                .submit_cpu(&self.scope, spec, |context| Initialize::new(context.memory().clone()))
        } else {
            let renderer = self.renderer.as_ref().ok_or("Installer renderer absent")?.clone();
            let pending = self.pending.as_ref().ok_or("Installer render request absent")?;
            let input = pending.input.clone();
            let identity = pending.identity;
            self.client.submit_cpu(&self.scope, spec, move |context| {
                Render::new(renderer, input, identity, context.memory().clone())
            })
        };
        let job = match result {
            Ok(job) => job,
            Err(
                AdmissionError::QueueSaturated { .. }
                | AdmissionError::CompletionSaturated
                | AdmissionError::MemorySaturated { .. },
            ) => return Ok(()),
            Err(cause) => return Err(cause.into()),
        };
        let request = if initializing { None } else { self.pending.take() };
        self.active = Some(Active { job, request });
        Ok(())
    }

    /// Stop admission and cancel the dedicated render scope. Continue routing terminal results
    /// until `settled` is true; cancellation alone never proves native/font work has stopped.
    /// # Errors
    /// Preserves visibility-generation exhaustion before cancellation.
    pub fn close(&mut self) -> Result {
        self.set_visible(false)?;
        self.closing = true;
        self.pending = None;
        self.scope.cancel();
        Ok(())
    }

    /// Observe actual work settlement independently of retained prepared-frame memory.
    pub fn settled(&self) -> bool {
        let state = self.scope.snapshot();
        self.closing
            && self.active.is_none()
            && state.queued == 0
            && state.running == 0
            && state.terminal_unobserved == 0
            && state.children_live == 0
    }

    /// Release idle resources only after all real render outcomes have been observed.
    /// The caller still owns execution shutdown and any published frame's native retirement.
    /// # Errors
    /// Rejects release while rendering work or unobserved terminal results remain.
    pub fn finish(&mut self) -> Result {
        if !self.settled() {
            return Err("Installer rendering has not settled".into());
        }
        self.renderer = None;
        self.inputs = None;
        Ok(())
    }
}
