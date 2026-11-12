// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Correlate native navigation with backend work without document serialization.
use super::*;
use std::time::Duration;
use sugarsubstitute_application::setup_flow::{Outcome, Request, RequestId, SetupFlow};

/// One native surface's navigation and the original identity of its outstanding operation.
pub(super) struct Navigation {
    pub(super) flow: SetupFlow,
    pending: Option<RequestId>,
}

impl Navigation {
    /// Retain the application-owned installation proposal without opening native resources.
    pub(super) fn new(install_root: String) -> Result<Self> {
        Ok(Self { flow: SetupFlow::new(install_root)?, pending: None })
    }

    /// Submit an application-admitted intent, retaining correlation before backend admission.
    pub(super) fn submit(&mut self, controller: &mut Controller, request: Request) -> Result<()> {
        if self.pending.is_some() || self.flow.is_closing() {
            return Err("Native installer already has outstanding work or is closing".into());
        }
        self.pending = Some(request.id);
        controller.accept(requests::Request {
            id: request.id.get().to_string(),
            command: request.operation,
        })
    }

    /// Apply the root's bounded backend batch and submit any dependent request admitted by the flow.
    /// Native retirement still consumes backend outcomes after presentation closes.
    pub(super) fn advance(
        &mut self,
        controller: &mut Controller,
        events: Vec<Event>,
        now: Duration,
    ) -> Result<()> {
        for event in events {
            self.receive(controller, event, now)?;
        }
        Ok(())
    }

    /// Match the original identity before updating native state; stale output never navigates.
    pub(super) fn receive(
        &mut self,
        controller: &mut Controller,
        event: Event,
        now: Duration,
    ) -> Result<()> {
        if self.flow.is_closing() {
            return Ok(());
        }
        let outcome = match event.kind {
            Kind::Diagnostic(_) => return Ok(()),
            Kind::Unavailable(_) => {
                self.pending = None;
                self.flow.unavailable();
                return Ok(());
            }
            Kind::Failed(_) if event.id.is_none() => {
                self.pending = None;
                self.flow.unavailable();
                return Ok(());
            }
            Kind::Progress(progress) => {
                let id = self.correlate(event.id.as_deref())?;
                self.flow.progress(id, progress.status, Some(progress.overall))?;
                return Ok(());
            }
            Kind::Completed(completed) => Outcome::Completed(completed.into_flow()?),
            Kind::Failed(message) | Kind::Cancelled(Some(message)) => Outcome::Failed(message),
            Kind::Cancelled(None) => Outcome::Cancelled,
        };
        let id = self.correlate(event.id.as_deref())?;
        let next = self.flow.receive(id, outcome, now)?;
        self.pending = None;
        if let Some(request) = next {
            self.submit(controller, request)?;
        }
        Ok(())
    }

    /// A numeric-looking wire identity cannot manufacture an application request identity.
    fn correlate(&self, id: Option<&str>) -> Result<RequestId> {
        self.pending
            .filter(|pending| id.is_some_and(|id| id == pending.get().to_string()))
            .ok_or_else(|| "Native installer result does not match outstanding work".into())
    }
}

#[cfg(test)]
mod tests;
