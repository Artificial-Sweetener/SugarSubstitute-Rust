// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serve `workflow.request`: provision a workflow's input surfaces, show them on the Input
//! route, and change their mask appearance, canvas size and hover emphasis.
//!
//! Sync and resize replies wait for their input work in a bounded hold that never occupies the
//! reply queue; a completed reply leaves the hold only while a reply slot is free. Requests that
//! arrive before the session has restored its archived documents wait, in order, until it has.
use super::input_protocol::{appearance, extent, failure, resize, synced};
use super::*;
use crate::native_canvas::session::inputs::{
    AppearanceChange, ComfyFile, InputRejection, InputReply, InputTicket, MaskChannel, MaskSpec,
    SurfaceBase, SurfaceSpec,
};
use std::collections::VecDeque;
use sugarsubstitute_contracts::canvas_native::CanvasNativeReplyPayload;
use sugarsubstitute_contracts::canvas_native::{
    CanvasFailure, CanvasFailureStatus, CanvasMaskChannel, CanvasMaskKey, CanvasWorkflowCommand,
    CanvasWorkflowMask, CanvasWorkflowRequest, CanvasWorkflowRoute, CanvasWorkflowSurface,
    CanvasWorkflowSurfaceBase,
};
use sugarsubstitute_contracts::canvas_surface::CanvasFileReference;
use sugarsubstitute_desktop::{CanvasNativeReply, CanvasNativeRequestStorage};
use sugarsubstitute_image_ops::Extent;
/// Workflow requests that may wait for restored documents or for their input work.
const MAX_HELD: usize = 16;
/// The page's answer when input work fails for a reason other than a typed rejection.
const PROVISION_FAILED: &str = "The canvas could not prepare this workflow's inputs";
/// The page's answer when the canvas memory budget cannot hold a settled reply.
const REPLY_REFUSED: &str = "The canvas cannot hold this workflow's reply now";
/// The page's answer when a settled key map cannot be encoded within the protocol's bounds.
const REPLY_UNENCODABLE: &str = "The workflow's input key map exceeds the canvas protocol";

/// One workflow request awaiting restored documents or its ticket's reply.
struct Held {
    identity: MainIdentity,
    request: CanvasWorkflowRequest,
    claim: MemoryClaim,
    storage: CanvasNativeRequestStorage,
    /// The input work it waits for; `None` while it waits for restored documents.
    ticket: Option<InputTicket>,
    reply: Option<InputReply>,
}

/// The output selection last offered, re-offered when the Output route has nothing parked.
#[derive(Clone)]
struct OfferedOutput {
    file: CanvasFileReference,
    compared: Option<CanvasFileReference>,
}

/// Held workflow requests and the output the Output route returns to.
#[derive(Default)]
pub(super) struct Workflow {
    held: VecDeque<Held>,
    output: Option<OfferedOutput>,
}

impl Workflow {
    /// Remember the output selection the Output route re-offers.
    pub(super) fn offered(
        &mut self,
        file: &CanvasFileReference,
        compared: Option<&CanvasFileReference>,
    ) {
        self.output = Some(OfferedOutput { file: file.clone(), compared: compared.cloned() });
    }

    /// Drop the held requests of a retired navigation with their lane credit.
    pub(super) fn retire(&mut self, identity: MainIdentity) {
        self.held.retain(|held| held.identity != identity);
    }

    /// Drop every held request at close.
    pub(super) fn close(&mut self) {
        self.held.clear();
    }
}

impl NativeSurface {
    /// Serve `workflow.request` for the session's input documents.
    pub(super) fn receive_workflow(
        &mut self,
        identity: MainIdentity,
        request: CanvasWorkflowRequest,
        claim: MemoryClaim,
        storage: CanvasNativeRequestStorage,
        session: Option<&mut Session>,
    ) -> Result<(), Box<dyn Error>> {
        let Some(session) = session.filter(|session| session.initialized()) else {
            let failure = failure(CanvasFailureStatus::Unavailable, "The canvas is not ready");
            return self.answer(identity, request.failure_reply(&failure)?, claim);
        };
        if !self.available(identity) {
            let failure = failure(CanvasFailureStatus::Unavailable, "No canvas backend");
            return self.answer(identity, request.failure_reply(&failure)?, claim);
        }
        let waiting = self.workflow.held.iter().any(|held| held.ticket.is_none());
        if waiting || !session.inputs_restored() {
            if self.workflow.held.len() >= MAX_HELD {
                let failure = failure(CanvasFailureStatus::Unavailable, "Too many requests wait");
                return self.answer(identity, request.failure_reply(&failure)?, claim);
            }
            let held = Held { identity, request, claim, storage, ticket: None, reply: None };
            self.workflow.held.push_back(held);
            return Ok(());
        }
        self.apply_workflow(identity, request, claim, storage, session)
    }

    /// Apply one workflow command now, answering it or holding it for its ticket.
    fn apply_workflow(
        &mut self,
        identity: MainIdentity,
        request: CanvasWorkflowRequest,
        claim: MemoryClaim,
        storage: CanvasNativeRequestStorage,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        let outcome = match request.command() {
            CanvasWorkflowCommand::Sync { workflow, surfaces } => {
                let specs = match self.surface_specs(identity, surfaces) {
                    Ok(specs) => specs,
                    Err(failure) => {
                        return self.answer(identity, request.failure_reply(&failure)?, claim);
                    }
                };
                session.sync_inputs(workflow.as_str(), specs).map(Some)
            }
            CanvasWorkflowCommand::Activate { workflow, route, surface, mask } => {
                match (route, surface) {
                    (CanvasWorkflowRoute::Input, Some(surface)) => session.activate_input(
                        workflow.as_str(),
                        surface.as_str(),
                        mask.as_ref().map(CanvasMaskKey::as_str),
                    ),
                    _ => {
                        if !session.show_output() {
                            self.offer_output_again(identity, session)?;
                        }
                        Ok(())
                    }
                }
                .map(|()| None)
            }
            CanvasWorkflowCommand::Appearance { workflow, masks } => {
                let changes = masks
                    .iter()
                    .map(|mask| {
                        Ok(AppearanceChange {
                            surface: Arc::from(mask.surface.as_str()),
                            mask: Arc::from(mask.mask.as_str()),
                            appearance: appearance(mask.color, mask.opacity)?,
                        })
                    })
                    .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
                session.set_input_appearance(workflow.as_str(), &changes).map(|()| None)
            }
            CanvasWorkflowCommand::Resize { workflow, surface, size, mode } => {
                let extent = Extent::new(size.width(), size.height())?;
                session
                    .resize_input(workflow.as_str(), surface.as_str(), extent, resize(*mode))
                    .map(Some)
            }
            CanvasWorkflowCommand::Hover { workflow, surface, mask } => session
                .hover_input_mask(
                    workflow.as_str(),
                    surface.as_str(),
                    mask.as_ref().map(CanvasMaskKey::as_str),
                )
                .map(|()| None),
        };
        match outcome {
            Ok(Some(ticket)) => {
                let ticket = Some(ticket);
                let held = Held { identity, request, claim, storage, ticket, reply: None };
                self.workflow.held.push_back(held);
                Ok(())
            }
            Ok(None) => {
                let payload = match request.command() {
                    CanvasWorkflowCommand::Activate { .. } => request.activated_reply()?,
                    _ => request.applied_reply()?,
                };
                self.answer(identity, payload, claim)
            }
            Err(cause) => {
                let answer = refusal(cause.as_ref());
                if cause.downcast_ref::<InputRejection>().is_none() {
                    tracing::warn!(
                        operation = "canvas_workflow_provision",
                        request_id = request.envelope().request_id(),
                        workflow = %workflow_of(request.command()),
                        cause = %cause,
                        "Workflow input work was refused to the page"
                    );
                }
                self.answer(identity, request.failure_reply(&answer)?, claim)
            }
        }
    }

    /// Answer a held sync or resize. A reply larger than the lane credit's inline bound is
    /// charged to the canvas memory budget; one the budget cannot hold is refused to the page.
    fn answer_settled(
        &mut self,
        held: Held,
        payload: CanvasNativeReplyPayload,
        session: &Session,
    ) -> Result<(), Box<dyn Error>> {
        let length = payload.as_bytes().len();
        if length <= CanvasNativeReply::INLINE_PAYLOAD_BYTES {
            return self.answer(held.identity, payload, held.claim);
        }
        let storage = session
            .preview_resident()
            .ok_or_else(|| Box::<dyn Error>::from("The canvas memory budget is closed"))
            .and_then(|memory| Ok(memory.claim(u64::try_from(length)?)?));
        match storage {
            Ok(storage) => {
                let reply =
                    CanvasNativeReply::with_storage(held.identity, payload, held.claim, storage)?;
                self.enqueue(DesktopStartupCommand::SendCanvasNative(reply))
            }
            Err(cause) => {
                tracing::warn!(
                    operation = "canvas_workflow_reply",
                    request_id = held.request.envelope().request_id(),
                    workflow = %workflow_of(held.request.command()),
                    reply_bytes = length as u64,
                    cause = %cause,
                    "The canvas memory budget refused a workflow reply"
                );
                let unheld = failure(CanvasFailureStatus::Unavailable, REPLY_REFUSED);
                self.answer(held.identity, held.request.failure_reply(&unheld)?, held.claim)
            }
        }
    }

    /// Admit held requests once documents are restored, then answer settled syncs and resizes
    /// while reply slots are free.
    pub(super) fn poll_workflow(&mut self, session: &mut Session) -> Result<(), Box<dyn Error>> {
        for reply in session.take_input_replies() {
            match self.workflow.held.iter_mut().find(|held| held.ticket == Some(reply.ticket)) {
                Some(held) => held.reply = Some(reply),
                None => tracing::debug!(
                    operation = "canvas_workflow_reply",
                    workflow = %reply.workflow,
                    "An input reply's request was withdrawn"
                ),
            }
        }
        while self.free_replies() > 0 {
            let Some(index) = self.workflow.held.iter().position(|held| held.reply.is_some())
            else {
                break;
            };
            let Some(held) = self.workflow.held.remove(index) else { break };
            let reply = held.reply.as_ref().ok_or("Held workflow reply disappeared")?;
            let encoded = synced(reply).and_then(|surfaces| {
                held.request.synced_reply(&surfaces).map_err(|cause| {
                    tracing::warn!(
                        operation = "canvas_workflow_reply",
                        request_id = held.request.envelope().request_id(),
                        workflow = %workflow_of(held.request.command()),
                        cause = %cause,
                        "A settled workflow key map could not be encoded"
                    );
                    failure(CanvasFailureStatus::Error, REPLY_UNENCODABLE)
                })
            });
            let payload = match encoded {
                Ok(payload) => payload,
                Err(failure) => held.request.failure_reply(&failure)?,
            };
            self.answer_settled(held, payload, session)?;
        }
        while session.inputs_restored() && self.free_replies() > 0 {
            let Some(index) = self.workflow.held.iter().position(|held| held.ticket.is_none())
            else {
                break;
            };
            let Some(held) = self.workflow.held.remove(index) else { break };
            if !self.available(held.identity) {
                let failure = failure(CanvasFailureStatus::Unavailable, "No canvas backend");
                self.answer(held.identity, held.request.failure_reply(&failure)?, held.claim)?;
                continue;
            }
            self.apply_workflow(held.identity, held.request, held.claim, held.storage, session)?;
        }
        Ok(())
    }

    /// Answer a held workflow request with `cancelled`; its input work still completes.
    /// Returns whether one matched.
    pub(super) fn cancel_workflow(
        &mut self,
        identity: MainIdentity,
        request_id: u64,
    ) -> Result<bool, Box<dyn Error>> {
        let Some(index) = self.workflow.held.iter().position(|held| {
            held.identity == identity && held.request.envelope().request_id() == request_id
        }) else {
            return Ok(false);
        };
        let held = self.workflow.held.remove(index).ok_or("Held workflow request disappeared")?;
        self.answer(identity, held.request.envelope().cancelled_reply()?, held.claim)?;
        Ok(true)
    }

    /// Reply slots not yet occupied.
    pub(super) fn free_replies(&self) -> usize {
        self.replies.iter().filter(|slot| slot.is_none()).count()
    }

    /// Show the output selection again when the Output route had nothing parked behind the
    /// input document.
    fn offer_output_again(
        &mut self,
        identity: MainIdentity,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        let Some(output) = self.workflow.output.clone() else { return Ok(()) };
        let (Some(target), Ok(compared_target)) = (
            super::operations::admitted_output(&output.file),
            output
                .compared
                .as_ref()
                .map(|file| super::operations::admitted_output(file).ok_or(()))
                .transpose(),
        ) else {
            return Ok(());
        };
        let retention =
            output.compared.is_none().then(|| self.retention_key(identity, &target)).flatten();
        if let Some(key) = &retention
            && session.reselect_retained(key)?
        {
            return Ok(());
        }
        let offered = self.offer_selection(
            identity,
            &output.file,
            output.compared.as_ref(),
            target,
            compared_target,
            session,
        )?;
        if offered == Some(true)
            && let Some(key) = retention
        {
            session.retain_selection(key);
        }
        Ok(())
    }

    /// Translate a sync's surfaces into session specs with inert acquisition adapters. Every
    /// reference is validated before any adapter exists.
    fn surface_specs(
        &self,
        identity: MainIdentity,
        surfaces: &[CanvasWorkflowSurface],
    ) -> Result<Vec<SurfaceSpec>, CanvasFailure> {
        let invalid = |_| failure(CanvasFailureStatus::Error, "An input file reference is invalid");
        for surface in surfaces {
            if let CanvasWorkflowSurfaceBase::File(file) = &surface.base {
                ComfyViewTarget::new(file).map_err(invalid)?;
            }
            for mask in surface.masks.iter().filter_map(|mask| mask.initial.as_ref()) {
                ComfyViewTarget::new(mask).map_err(invalid)?;
            }
        }
        surfaces
            .iter()
            .map(|surface| {
                Ok(SurfaceSpec {
                    key: Arc::from(surface.key.as_str()),
                    label: Arc::from(surface.label.as_str()),
                    base: match &surface.base {
                        CanvasWorkflowSurfaceBase::File(file) => {
                            SurfaceBase::File(self.input_file(identity, file)?)
                        }
                        CanvasWorkflowSurfaceBase::Blank(size) => {
                            SurfaceBase::Blank(extent(*size)?)
                        }
                    },
                    masks: surface
                        .masks
                        .iter()
                        .map(|mask| self.mask_spec(identity, mask))
                        .collect::<Result<_, _>>()?,
                })
            })
            .collect()
    }

    fn mask_spec(
        &self,
        identity: MainIdentity,
        mask: &CanvasWorkflowMask,
    ) -> Result<MaskSpec, CanvasFailure> {
        Ok(MaskSpec {
            key: Arc::from(mask.key.as_str()),
            label: Arc::from(mask.label.as_str()),
            appearance: appearance(mask.color, mask.opacity)
                .map_err(|_| failure(CanvasFailureStatus::Error, "A mask appearance is invalid"))?,
            channel: match mask.channel {
                CanvasMaskChannel::Red => MaskChannel::Red,
                CanvasMaskChannel::Green => MaskChannel::Green,
                CanvasMaskChannel::Blue => MaskChannel::Blue,
                CanvasMaskChannel::Alpha => MaskChannel::Alpha,
            },
            initial: mask
                .initial
                .as_ref()
                .map(|file| self.input_file(identity, file))
                .transpose()?,
        })
    }

    /// Bind one ComfyUI file reference to an inert adapter on the current backend grant. Its
    /// reference text is the canonical `/view` target, so equal files compare equal.
    fn input_file(
        &self,
        identity: MainIdentity,
        file: &CanvasFileReference,
    ) -> Result<ComfyFile, CanvasFailure> {
        let target = ComfyViewTarget::new(file).map_err(|_| {
            failure(CanvasFailureStatus::Error, "An input file reference is invalid")
        })?;
        let reference: Arc<str> = Arc::from(target.as_str());
        #[cfg(test)]
        if let Some(output) = &self.fixture_output {
            let path = super::operations::fixture_file(output, file)
                .ok_or_else(|| failure(CanvasFailureStatus::Error, "No such fixture file"))?;
            let acquirer = Arc::new(super::fixture_file::FixtureFile::new(path));
            return Ok(ComfyFile { reference, acquirer });
        }
        let backend = self
            .backend
            .filter(|backend| backend.identity == identity)
            .ok_or_else(|| failure(CanvasFailureStatus::Unavailable, "No canvas backend"))?;
        let owner = super::super::comfy_source::ComfySource::new(backend, target, None)
            .map_err(|_| failure(CanvasFailureStatus::Error, "An input file is unreachable"))?;
        Ok(ComfyFile { reference, acquirer: Arc::new(owner) })
    }
}

/// The workflow a command addresses.
fn workflow_of(command: &CanvasWorkflowCommand) -> &str {
    match command {
        CanvasWorkflowCommand::Sync { workflow, .. }
        | CanvasWorkflowCommand::Activate { workflow, .. }
        | CanvasWorkflowCommand::Appearance { workflow, .. }
        | CanvasWorkflowCommand::Resize { workflow, .. }
        | CanvasWorkflowCommand::Hover { workflow, .. } => workflow.as_str(),
    }
}

/// The page's typed answer to input work that did not start or did not complete. A typed
/// rejection keeps its own status and message; any other failure is an actionable error, so a
/// failed provisioning never ends the application.
fn refusal(cause: &(dyn Error + 'static)) -> CanvasFailure {
    match cause.downcast_ref::<InputRejection>() {
        Some(InputRejection::Invalid(message)) => failure(CanvasFailureStatus::Error, message),
        Some(rejection @ InputRejection::Unknown) => {
            failure(CanvasFailureStatus::Stale, &rejection.to_string())
        }
        Some(InputRejection::Unavailable(message)) => {
            failure(CanvasFailureStatus::Unavailable, message)
        }
        None => failure(CanvasFailureStatus::Error, PROVISION_FAILED),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_input_work_is_answered_with_its_typed_status_or_an_actionable_error() {
        let refused = |cause: Box<dyn Error>| refusal(cause.as_ref());
        assert_eq!(
            refused(Box::new(InputRejection::Unavailable("The canvas is closing"))),
            failure(CanvasFailureStatus::Unavailable, "The canvas is closing")
        );
        assert_eq!(
            refused(Box::new(InputRejection::Invalid("Input surface keys repeat"))),
            failure(CanvasFailureStatus::Error, "Input surface keys repeat")
        );
        assert_eq!(refused(Box::new(InputRejection::Unknown)).status, CanvasFailureStatus::Stale);
        assert_eq!(
            refused("Input document identity exhausted".into()),
            failure(CanvasFailureStatus::Error, PROVISION_FAILED)
        );
    }
}
