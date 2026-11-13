// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Revalidate the selected original before an Input-to-Output route can reuse any pixels.
use super::*;

/// Correlation from an admitted Select, sharing its bounded host source allocation.
#[derive(Clone)]
pub(super) struct Selection {
    pub(super) identity: MainIdentity,
    pub(super) request: Box<CanvasSurfaceRequest>,
    pub(super) _memory: Arc<MemoryClaim>,
}

/// One actual held workflow route and its exact previous output selection.
pub(in crate::startup) struct OutputReplay {
    pub(in crate::startup) identity: MainIdentity,
    request_id: u64,
    pub(in crate::startup) selection: Box<CanvasSurfaceRequest>,
    _memory: Arc<MemoryClaim>,
}

impl NativeSurface {
    /// The output the current attachment offered. A released or replaced attachment took its
    /// offer with it, so an attachment that offered nothing has no output to show or replay.
    pub(in crate::startup::canvas::surface) fn offered_output(&self) -> Option<&OfferedOutput> {
        self.binding.as_ref()?.offered.as_ref()
    }

    /// Descriptor rediscovery may change a path while these exact output UUIDs stay the same.
    pub(in crate::startup::canvas::surface) fn same_originals(
        &self,
        request: &CanvasSurfaceRequest,
    ) -> bool {
        let Some(previous) = self.offered_output().and_then(|output| output.correlation.as_ref())
        else {
            return false;
        };
        match (previous.request.operation(), request.operation()) {
            (
                CanvasSurfaceOperation::Select { item_id: old, comparison: old_compared, .. },
                CanvasSurfaceOperation::Select { item_id, comparison, .. },
            ) => {
                old == item_id
                    && old_compared.as_ref().map(|value| &value.item_id)
                        == comparison.as_ref().map(|value| &value.item_id)
            }
            _ => false,
        }
    }

    /// Parked documents require the same current original decision as a new acquisition. Only
    /// the current attachment's own selection can need it; an attachment that offered no
    /// output shows the Output route without revalidating anything.
    pub(super) fn required_replay(&self, command: &CanvasWorkflowCommand) -> Option<Selection> {
        match command {
            CanvasWorkflowCommand::Activate { route, surface, .. }
                if !matches!((route, surface), (CanvasWorkflowRoute::Input, Some(_))) =>
            {
                self.offered_output()?.correlation.clone()
            }
            _ => None,
        }
    }

    /// Borrow an owned route only after native input restoration has settled.
    pub(in crate::startup::canvas) fn output_replay(
        &self,
        session: &Session,
    ) -> Option<OutputReplay> {
        if !session.inputs_restored() || self.free_replies() == 0 {
            return None;
        }
        let held = self.workflow.held.iter().find(|held| held.replay.is_some())?;
        let selection = held.replay.as_ref()?;
        Some(OutputReplay {
            identity: held.identity,
            request_id: held.request.envelope().request_id(),
            selection: selection.request.clone(),
            _memory: selection._memory.clone(),
        })
    }

    /// Consume the exact held route after the root independently classified every original.
    /// Missing/refused sources and superseded native attachments preserve the prior document.
    pub(in crate::startup::canvas) fn resume_output_replay(
        &mut self,
        replay: OutputReplay,
        source: Option<super::super::super::session_source::SessionSource>,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        let Some(index) = self.workflow.held.iter().position(|held| {
            held.replay.is_some()
                && held.identity == replay.identity
                && held.request.envelope().request_id() == replay.request_id
        }) else {
            return Ok(());
        };
        let held = self.workflow.held.remove(index).ok_or("Held output route disappeared")?;
        let attached = self.binding.as_ref().is_some_and(|binding| {
            binding.identity == replay.identity && binding.address == *replay.selection.address()
        });
        if !attached {
            // The attachment that offered this selection was released or replaced. Its offer
            // ended with it, so the route is admitted again against the current attachment.
            self.workflow.held.insert(index, Held { replay: None, ..held });
            return Ok(());
        }
        let current = self.offered_output().and_then(|output| output.correlation.as_ref());
        let admitted = current.is_some_and(|current| {
            current.identity == replay.identity && current.request == replay.selection
        }) && self.available(replay.identity)
            && source.as_ref().is_some_and(|source| !source.unavailable());
        if !admitted {
            let failure =
                failure(CanvasFailureStatus::Unavailable, "The selected output is unavailable");
            return self.answer(held.identity, held.request.failure_reply(&failure)?, held.claim);
        }
        let key = source.as_ref().and_then(|source| source.retention_key());
        let hashes = source.as_ref().and_then(|source| source.original_hashes());
        self.session_source = source;
        let result = match key {
            Some(key) => {
                let qualified = hashes
                    .as_deref()
                    .ok_or_else(|| "Verified original hashes absent".into())
                    .and_then(|hashes| session.retain_output_original(&key, hashes));
                let offered = match qualified {
                    Ok(true) => Ok(()),
                    Ok(false) => self.offer_output_again(replay.identity, session),
                    Err(cause) => Err(cause),
                };
                if offered.is_ok() {
                    session.show_output_original(key);
                }
                offered
            }
            None if session.show_output() => Ok(()),
            None => self.offer_output_again(replay.identity, session),
        };
        self.session_source = None;
        match result {
            Ok(()) => self.answer(held.identity, held.request.activated_reply()?, held.claim),
            Err(cause) => {
                tracing::warn!(operation="canvas_output_replay_refused",request_id=replay.request_id,cause=%cause,"Native output route retained its prior document");
                let failure =
                    failure(CanvasFailureStatus::Unavailable, "The selected output is unavailable");
                self.answer(held.identity, held.request.failure_reply(&failure)?, held.claim)
            }
        }
    }
}

impl NativeSurface {
    /// Show the output selection again when the Output route had nothing parked behind the
    /// input document.
    pub(super) fn offer_output_again(
        &mut self,
        identity: MainIdentity,
        session: &mut Session,
    ) -> Result<(), Box<dyn Error>> {
        let Some(output) = self.offered_output().cloned() else { return Ok(()) };
        let (Some(target), Ok(compared_target)) = (
            super::super::operations::admitted_output(&output.file),
            output
                .compared
                .as_ref()
                .map(|file| super::super::operations::admitted_output(file).ok_or(()))
                .transpose(),
        ) else {
            return Ok(());
        };
        let verified =
            self.session_source.as_ref().is_some_and(|source| source.retention_key().is_some());
        let retention = (output.compared.is_none() || verified)
            .then(|| self.retention_key(identity, &target))
            .flatten();
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
        if offered != Some(true) {
            return Err("Native output source admission refused".into());
        }
        if let Some(key) = retention {
            session.retain_selection(key);
        }
        Ok(())
    }
}
