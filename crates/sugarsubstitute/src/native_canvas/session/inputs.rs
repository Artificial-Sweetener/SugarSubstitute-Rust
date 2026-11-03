// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the input documents a workflow's image and mask loaders edit on the Input route.
//!
//! Each synced surface owns one authoritative document: a locked base layer (a ComfyUI input
//! image or a blank backing) and one coverage layer per mask. Documents are created and
//! re-based by owned jobs, survive every route change and memory decision, and leave only
//! through a later sync that no longer lists them or through session close. Surface keys, mask
//! keys, document identities and layer identities stay stable for the surface's lifetime.
//!
//! Structural changes a workflow drives (masks added or removed, labels, synced appearance,
//! replaced base images and loaded mask files) are provisioned into every history state, so
//! undo never contradicts the workflow. Appearance commands and canvas geometry changes are
//! ordinary undoable steps; hover emphasis is presentation only.
mod apply;
mod archive;
mod build;
mod capture;
mod contract;
mod edits;
mod jobs;
mod memory;
mod reconcile;
mod route;
mod sync;

use super::*;
pub(crate) use crate::native_canvas::work::capture::CapturePixels;
pub(crate) use capture::{
    CaptureEncoder, CaptureRejection, CaptureTarget, CaptureTicket, EncodedCapture, InputCapture,
    capture_extent,
};
pub(crate) use contract::*;
use std::collections::VecDeque;
use sugarsubstitute_canvas::{Layer, LayerId, MaskAppearance};
use sugarsubstitute_image_ops::{CoverageResampling, Extent};

/// High bit reserved for input document identities; output documents are identified by their
/// selection, which counts up from one and never reaches it.
const IDENTITY_NAMESPACE: u64 = 1 << 63;
/// The base layer of every input document.
const BASE_LAYER: u64 = 1;
/// Largest surface set one workflow sync may carry.
const MAX_SURFACES: usize = 64;
/// Largest mask set one surface may carry.
const MAX_MASKS: usize = 32;

/// The base a document actually holds, compared with what the workflow asks for.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Base {
    File(Arc<str>),
    Blank(Extent),
}

/// One mask's synced state.
struct Mask {
    key: Arc<str>,
    label: Arc<str>,
    layer: LayerId,
    /// Last synced appearance and whether the document still has to receive it.
    appearance: MaskAppearance,
    appearance_pending: bool,
    channel: MaskChannel,
    /// Latest requested initial file and the one the mask last received or rejected.
    desired_initial: Option<Arc<str>>,
    loaded_initial: Option<Arc<str>>,
    outcome: Option<InitialOutcome>,
}

/// One surface's synced state and, while it is not shown, its document.
struct Surface {
    key: Arc<str>,
    label: Arc<str>,
    identity: InputDocumentId,
    desired_base: Base,
    loaded_base: Option<Base>,
    masks: Vec<Mask>,
    next_layer: u64,
    /// The document while parked here; `None` while it loads or is the session's current one.
    document: Option<Box<document::DocumentState>>,
    /// Archived content awaiting the base image the next sync acquires.
    restored: Option<sugarsubstitute_canvas::ArchivedDocument>,
    loaded: bool,
    failure: Option<InputFailure>,
}

/// A request awaiting the settlement of every surface it named.
struct PendingTicket {
    id: u64,
    workflow: Arc<str>,
    keys: Vec<Arc<str>>,
    failures: Vec<(Arc<str>, InputFailure)>,
}

/// Disposals that must run on the retirement worker.
#[derive(Default)]
pub(super) struct Retiring {
    documents: Vec<Box<document::DocumentState>>,
    scenes: Vec<Scene>,
    sources: Vec<reconcile::Replaced>,
    products: Vec<build::Product>,
    surfaces: Vec<Arc<CpuSurface>>,
    gpu_surfaces: Vec<GpuSurface>,
}

impl Retiring {
    fn is_empty(&self) -> bool {
        self.documents.is_empty()
            && self.scenes.is_empty()
            && self.sources.is_empty()
            && self.products.is_empty()
            && self.surfaces.is_empty()
            && self.gpu_surfaces.is_empty()
    }
}

/// The session's input documents, their pending workflow work and the output parked behind
/// a shown input document.
#[derive(Default)]
pub(super) struct InputDocuments {
    workflows: Vec<(Arc<str>, Vec<Surface>)>,
    next_identity: u64,
    next_ticket: u64,
    next_edit: u64,
    queue: VecDeque<jobs::Task>,
    running: Option<jobs::Running>,
    tickets: Vec<PendingTicket>,
    replies: VecDeque<InputReply>,
    /// Inert adapters awaiting worker retirement of their native custody.
    unsettled: VecDeque<Arc<dyn acquisition::SourceAcquirer>>,
    retiring: Retiring,
    route: Option<route::Pending>,
    captures: capture::Captures,
    persistence: Option<archive::Persistence>,
    /// The output document shown before the current input document, restored on Output.
    pub(super) parked_output: Option<Box<document::DocumentState>>,
}

impl InputDocuments {
    fn surfaces(&self, workflow: &str) -> Option<&Vec<Surface>> {
        self.workflows.iter().find(|(id, _)| &**id == workflow).map(|(_, surfaces)| surfaces)
    }

    fn surface(&self, workflow: &str, key: &str) -> Option<&Surface> {
        self.surfaces(workflow)?.iter().find(|surface| &*surface.key == key)
    }

    fn by_identity(&self, identity: InputDocumentId) -> Option<&Surface> {
        self.workflows
            .iter()
            .flat_map(|(_, surfaces)| surfaces)
            .find(|surface| surface.identity == identity)
    }

    fn by_identity_mut(&mut self, identity: InputDocumentId) -> Option<&mut Surface> {
        self.workflows
            .iter_mut()
            .flat_map(|(_, surfaces)| surfaces)
            .find(|surface| surface.identity == identity)
    }

    /// Whether queued or running work will still change this surface.
    fn busy(&self, identity: InputDocumentId) -> bool {
        self.queue.iter().any(|task| task.surface == identity)
            || self.running.as_ref().is_some_and(|running| running.surface == identity)
    }

    /// The parked input document whose snapshots carry `identity`.
    pub(super) fn parked_mut(&mut self, identity: u64) -> Option<&mut document::DocumentState> {
        self.workflows
            .iter_mut()
            .flat_map(|(_, surfaces)| surfaces)
            .find(|surface| surface.identity.get() == identity)?
            .document
            .as_deref_mut()
    }

    /// Return an input document to its surface, or retire it when its surface is gone.
    pub(super) fn stow(&mut self, document: Box<document::DocumentState>) {
        let identity = document.input;
        match identity.and_then(|identity| self.by_identity_mut(identity)) {
            Some(surface) => surface.document = Some(document),
            None => self.retiring.documents.push(document),
        }
    }

    /// Stop an unused adapter and keep it until its native custody has retired.
    fn release(&mut self, acquirer: Arc<dyn acquisition::SourceAcquirer>) {
        acquirer.cancel();
        if !acquirer.settled() {
            self.unsettled.push_back(acquirer);
        }
    }

    /// Next edit identity for appearance steps, kept apart from tool gesture identities.
    fn edit_identity(&mut self) -> Result<sugarsubstitute_canvas::EditSessionId, Box<dyn Error>> {
        self.next_edit = self.next_edit.checked_add(1).ok_or("Input edit identity exhausted")?;
        Ok(sugarsubstitute_canvas::EditSessionId::new(IDENTITY_NAMESPACE | self.next_edit)?)
    }

    /// Whether parked documents, pending work or disposals still hold resources.
    pub(super) fn has_resources(&self) -> bool {
        self.workflows
            .iter()
            .any(|(_, surfaces)| surfaces.iter().any(|surface| surface.document.is_some()))
            || self.parked_output.is_some()
            || !self.retiring.is_empty()
            || !self.queue.is_empty()
            || self.captures.holds()
    }

    /// Whether no work is queued or running, no route change waits and every adapter retired.
    #[cfg(test)]
    pub(super) fn idle(&self) -> bool {
        self.queue.is_empty()
            && self.running.is_none()
            && self.unsettled.is_empty()
            && self.route.is_none()
            && !self.captures.in_flight()
    }

    /// Whether a job is running.
    #[cfg(test)]
    pub(super) fn running(&self) -> bool {
        self.running.is_some()
    }

    /// Whether running work or adapters still await their terminal outcomes.
    pub(super) fn in_flight(&self) -> bool {
        self.running.is_some()
            || !self.unsettled.is_empty()
            || self.captures.in_flight()
            || self.persistence.as_ref().is_some_and(archive::Persistence::in_flight)
    }
}

impl Surface {
    fn mask(&self, key: &str) -> Option<&Mask> {
        self.masks.iter().find(|mask| &*mask.key == key)
    }

    /// The reply entry for this surface given its current document.
    fn outcome(&self, document: Option<&document::DocumentState>) -> SurfaceOutcome {
        if let Some(failure) = &self.failure {
            return SurfaceOutcome::Failed { key: self.key.clone(), failure: failure.clone() };
        }
        let snapshot = document
            .and_then(|document| document.scene.as_ref())
            .map(|scene| scene.document.snapshot());
        let Some(snapshot) = snapshot.filter(|_| self.loaded) else {
            return SurfaceOutcome::Failed {
                key: self.key.clone(),
                failure: InputFailure::message("The input document is unavailable"),
            };
        };
        let extent = snapshot.extent();
        SurfaceOutcome::Synced(SyncedSurface {
            key: self.key.clone(),
            document: self.identity,
            geometry: self.identity.geometry(extent),
            extent,
            masks: self
                .masks
                .iter()
                .map(|mask| SyncedMask {
                    key: mask.key.clone(),
                    layer: mask.layer,
                    initial: mask.outcome.clone(),
                })
                .collect(),
        })
    }
}

impl Session {
    /// The input document with `identity`, wherever it is held: current or parked.
    fn input_document(&self, identity: InputDocumentId) -> Option<&document::DocumentState> {
        if self.document.input == Some(identity) {
            return Some(&self.document);
        }
        self.inputs.by_identity(identity)?.document.as_deref()
    }

    /// Mutable access to the input document with `identity`, and whether it is current.
    pub(super) fn input_document_mut(
        &mut self,
        identity: InputDocumentId,
    ) -> Option<(&mut document::DocumentState, bool)> {
        if self.document.input == Some(identity) {
            return Some((&mut self.document, true));
        }
        self.inputs
            .by_identity_mut(identity)?
            .document
            .as_deref_mut()
            .map(|document| (document, false))
    }

    /// Whether the current document is an input document.
    pub(crate) fn input_shown(&self) -> bool {
        self.document.input.is_some()
    }

    /// The input document the Input route shows, once its swap has completed.
    pub(crate) fn shown_input(&self) -> Option<ShownInput> {
        let document = self.document.input?;
        self.inputs.by_identity(document).filter(|surface| surface.loaded)?;
        let snapshot = self.document.scene.as_ref()?.document.snapshot();
        Some(ShownInput { document, snapshot, target: self.document.tools.target() })
    }

    /// The content revision of every loaded synced document, current or parked.
    pub(crate) fn input_revisions(&self) -> Vec<(InputDocumentId, u64)> {
        self.inputs
            .workflows
            .iter()
            .flat_map(|(_, surfaces)| surfaces)
            .filter(|surface| surface.loaded)
            .filter_map(|surface| {
                let scene = self.input_document(surface.identity)?.scene.as_ref()?;
                Some((surface.identity, scene.document.snapshot().revision()))
            })
            .collect()
    }

    /// Read a synced input document for capture, current or parked. With `revision`, a
    /// document whose revision differs, or that queued workflow work is about to change, is
    /// reported stale.
    pub(crate) fn input_snapshot(
        &self,
        document: InputDocumentId,
        revision: Option<u64>,
    ) -> InputSnapshot {
        let Some(surface) = self.inputs.by_identity(document).filter(|surface| surface.loaded)
        else {
            return InputSnapshot::Unavailable;
        };
        let Some(snapshot) = self
            .input_document(document)
            .and_then(|document| document.scene.as_ref())
            .map(|scene| scene.document.snapshot())
        else {
            return InputSnapshot::Unavailable;
        };
        if self.inputs.busy(surface.identity)
            || revision.is_some_and(|revision| revision != snapshot.revision())
        {
            InputSnapshot::Stale
        } else {
            InputSnapshot::Current(snapshot)
        }
    }

    /// The current content of a synced document, whatever work is pending on it.
    #[cfg(test)]
    pub(crate) fn input_content(&self, document: InputDocumentId) -> Option<DocumentSnapshot> {
        Some(self.input_document(document)?.scene.as_ref()?.document.snapshot())
    }

    /// Take every reply completed since the last call, oldest first.
    pub(crate) fn take_input_replies(&mut self) -> Vec<InputReply> {
        self.inputs.replies.drain(..).collect()
    }

    /// Reply to every ticket whose surfaces have no queued or running work left.
    fn settle_input_tickets(&mut self) {
        let mut index = 0;
        while index < self.inputs.tickets.len() {
            let ticket = &self.inputs.tickets[index];
            let busy = ticket.keys.iter().any(|key| {
                self.inputs
                    .surface(&ticket.workflow, key)
                    .is_some_and(|surface| self.inputs.busy(surface.identity))
            });
            if busy {
                index += 1;
                continue;
            }
            let ticket = self.inputs.tickets.remove(index);
            let surfaces = ticket
                .keys
                .iter()
                .map(|key| {
                    if let Some((_, failure)) =
                        ticket.failures.iter().find(|(failed, _)| failed == key)
                    {
                        return SurfaceOutcome::Failed {
                            key: key.clone(),
                            failure: failure.clone(),
                        };
                    }
                    match self.inputs.surface(&ticket.workflow, key) {
                        Some(surface) => surface.outcome(self.input_document(surface.identity)),
                        None => SurfaceOutcome::Failed {
                            key: key.clone(),
                            failure: InputFailure::message("A later sync retired the surface"),
                        },
                    }
                })
                .collect();
            self.inputs.replies.push_back(InputReply {
                ticket: InputTicket(ticket.id),
                workflow: ticket.workflow,
                surfaces,
            });
        }
    }

    /// Register a request that replies once `keys` of `workflow` have settled.
    fn input_ticket(
        &mut self,
        workflow: Arc<str>,
        keys: Vec<Arc<str>>,
    ) -> Result<InputTicket, Box<dyn Error>> {
        self.inputs.next_ticket =
            self.inputs.next_ticket.checked_add(1).ok_or("Input ticket identity exhausted")?;
        let id = self.inputs.next_ticket;
        self.inputs.tickets.push(PendingTicket { id, workflow, keys, failures: Vec::new() });
        Ok(InputTicket(id))
    }

    /// Retire released adapters and hand input disposals to the retirement worker once its
    /// deferred slot is free.
    pub(super) fn advance_input_retirement(&mut self) -> Result<(), Box<dyn Error>> {
        // Adapters retire one at a time through the session's acquisition retirement, whose
        // job is admitted at once so its completion wakes the session for the next one.
        while self.acquisition_retirement.is_none()
            && let Some(owner) = self.inputs.unsettled.pop_front()
        {
            self.retain_acquirer(owner)?;
            self.advance_acquirer_retirement()?;
        }
        if self.deferred_retirement.is_some() || self.inputs.retiring.is_empty() {
            return Ok(());
        }
        let retiring = std::mem::take(&mut self.inputs.retiring);
        self.retire(Retired { _inputs: Some(retiring), ..Default::default() })
    }

    /// Transfer every input document, parked output and pending disposal for close.
    pub(super) fn take_input_documents(&mut self) -> Option<Retiring> {
        let mut retiring = std::mem::take(&mut self.inputs.retiring);
        for (_, surfaces) in self.inputs.workflows.drain(..) {
            retiring.documents.extend(surfaces.into_iter().filter_map(|surface| surface.document));
        }
        retiring.documents.extend(self.inputs.parked_output.take());
        self.inputs.route = None;
        (!retiring.is_empty()).then_some(retiring)
    }

    /// Stop input work for close: cancel the running job and release every queued adapter.
    pub(super) fn close_inputs(&mut self) {
        self.inputs.captures.close();
        if let Some(persistence) = &mut self.inputs.persistence {
            persistence.close();
        }
        self.close_input_documents();
        if let Some(running) = &self.inputs.running {
            running.job.cancel();
            running.acquirers.iter().for_each(|owner| owner.cancel());
        }
        while let Some(task) = self.inputs.queue.pop_front() {
            for owner in task.acquirers() {
                self.inputs.release(owner);
            }
        }
        self.inputs.route = None;
    }
}
