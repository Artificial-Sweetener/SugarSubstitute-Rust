// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep one optional preview with the actual registered child, original claim and campaign.
#[cfg(test)]
#[path = "preview_hooks_tests.rs"]
mod tests;
use super::super::preview::{Preview, PreviewError};
use super::ChildQualification;
use crate::{job_sequence::JobSequence, production::qualification::PreviewIntent};
use std::{error::Error, fmt};
use sugarsubstitute_application::startup::StartupSession;
use sugarsubstitute_contracts::{execution::*, lifecycle::RunKey, startup::MainIdentity};
use sugarsubstitute_desktop::{DesktopStartupCommand, DesktopStartupObservation};
use sugarsubstitute_execution::{
    DesktopCaptureSession, ExecutionClient, InheritedCaptureAttachInput, PreviewArmingError,
    PreviewSourceLoan,
};

/// Only passive intent exists until the original capture preclaim and real registration.
#[derive(Default)]
pub(super) struct PreviewHooks {
    intent: Option<PreviewIntent>,
    source: Option<PreviewSourceLoan>,
    driver: Option<Preview>,
    rejected: Option<DesktopStartupObservation>,
    failure: Option<PreviewHookCause>,
    until: Option<Moment>,
    requested: bool,
    declared: bool,
    attempted: bool,
    closing: bool,
}
#[derive(Debug)]
enum PreviewHookCause {
    Capacity,
    Declaration(AdmissionError),
    Arming(PreviewArmingError),
    Driver(PreviewError),
}
impl fmt::Display for PreviewHookCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capacity => formatter.write_str("Original preview controller credit rejected"),
            Self::Declaration(cause) => cause.fmt(formatter),
            Self::Arming(cause) => cause.fmt(formatter),
            Self::Driver(cause) => cause.fmt(formatter),
        }
    }
}
impl Error for PreviewHookCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Capacity => None,
            Self::Declaration(cause) => Some(cause),
            Self::Arming(cause) => Some(cause),
            Self::Driver(cause) => Some(cause),
        }
    }
}
impl ChildQualification {
    /// Declare the entire inline qualifier and sole path before original four-MiB admission.
    /// Rejection leaves the identical inherited input untouched, retains its original cause,
    /// disables preview and leaves passive path custody in the earlier owned argument intent.
    /// The ordinary capture may then proceed with its unchanged no-preview declaration.
    pub(in crate::production::child) fn declare_preview_storage(
        &mut self,
        input: &mut InheritedCaptureAttachInput,
    ) {
        let Some(intent) = self.intent.preview() else {
            return;
        };
        self.preview.requested = true;
        let Some(bytes) = size_of::<Self>().checked_add(intent.allocated_bytes()) else {
            self.preview.record(PreviewHookCause::Capacity);
            return;
        };
        match input.reserve_preview_control_storage(bytes) {
            Ok(()) => self.preview.declared = true,
            Err(cause) => self.preview.record(PreviewHookCause::Declaration(cause)),
        }
    }

    /// Borrow only after Bootstrap observes actual capture readiness and parent registration.
    /// Core authenticates the real Session, scopes, creator and immutable storage declaration.
    /// This is the sole campaign conversion; a rejection never retries or resamples it.
    pub(in crate::production::child) fn prepare_preview(
        &mut self,
        client: &ExecutionClient,
        native_scope: &Scope,
        session: &DesktopCaptureSession,
        run: RunKey,
    ) {
        if !self.preview.requested
            || !self.preview.declared
            || self.preview.attempted
            || self.preview.closing
            || self.preview.failure.is_some()
        {
            return;
        }
        self.preview.attempted = true;
        let Some(intent) = self.intent.preview() else {
            self.preview.record(PreviewHookCause::Capacity);
            return;
        };
        let source =
            match client.borrow_preview_source(session, native_scope, run, intent.deadline()) {
                Ok(source) => source,
                Err(cause) => {
                    self.preview.record(PreviewHookCause::Arming(cause));
                    return;
                }
            };
        self.preview.until = Some(source.original_deadline());
        self.preview.source = Some(source);
        self.preview.intent = self.intent.take_preview();
        match Preview::new(&mut self.preview.intent, run, &mut self.preview.source) {
            Ok(driver) => self.preview.driver = Some(driver),
            Err(cause) => self.preview.record(PreviewHookCause::Driver(cause)),
        }
    }

    /// Request only after the unchanged StartupPolicy accepted genuine MainRevealed as Running.
    /// The existing ordinary scope funds persistence; the native scope fences source lifetime.
    pub(in crate::production::child) fn preview_after_main_revealed(
        &mut self,
        identity: MainIdentity,
        session: &StartupSession,
        ordinary_scope: &Scope,
        native_scope: &Scope,
        now: Moment,
    ) -> Option<DesktopStartupCommand> {
        let driver = self.preview.driver.as_mut()?;
        match driver.after_main_revealed(identity, session, ordinary_scope, native_scope, now) {
            Ok(command) => command,
            Err(cause) => {
                self.preview.record(PreviewHookCause::Driver(cause));
                None
            }
        }
    }

    /// Intercept only actual preview kinds; ordinary observations retain the same owner and route.
    /// Invalid payloads use one retained slot. A full slot explicitly disposes an already
    /// published passive payload with a categorical diagnostic; no callback, job or file is
    /// declared retired by that disposition. Their actual obligations stay with Desktop/Driver.
    pub(in crate::production::child) fn accept_preview(
        &mut self,
        observation: &mut Option<DesktopStartupObservation>,
        client: &ExecutionClient,
        ordinary_scope: &Scope,
        jobs: &JobSequence,
    ) -> bool {
        if !is_preview(observation.as_ref()) {
            return false;
        }
        if let Some(driver) = &mut self.preview.driver {
            match driver.accept(observation, client, ordinary_scope, jobs) {
                Ok(true) => return true,
                Ok(false) => {}
                Err(cause) => self.preview.record(PreviewHookCause::Driver(cause)),
            }
            if observation.is_none() {
                return true;
            }
        }
        self.retain_unarmed_preview(observation)
    }

    /// Keep or explicitly dispose only an already published optional payload without a worker.
    pub(in crate::production::child) fn retain_unarmed_preview(
        &mut self,
        observation: &mut Option<DesktopStartupObservation>,
    ) -> bool {
        if !is_preview(observation.as_ref()) {
            return false;
        }
        if let Some(driver) = &mut self.preview.driver {
            match driver.reject_retained(observation) {
                Ok(true) => return true,
                Ok(false) => {}
                Err(cause) => self.preview.record(PreviewHookCause::Driver(cause)),
            }
        } else if self.preview.rejected.is_none() {
            self.preview.rejected = observation.take();
            return true;
        }
        if let Some(rejected) = observation.take() {
            tracing::warn!(
                operation = "dispose_unarmed_main_preview",
                kind = preview_kind(&rejected),
                "Optional published payload explicitly disposed after bounded owner rejection"
            );
            drop(rejected);
        }
        true
    }

    /// Deliver the same immutable notifications before any optional failure is observed.
    pub(in crate::production::child) fn capture_preview(&mut self, batch: &CompletionBatch) {
        if let Some(driver) = &mut self.preview.driver {
            driver.capture(batch);
        }
    }

    /// Observe actual terminal/disposal using the existing clock, never a renewed cutoff.
    pub(in crate::production::child) fn poll_preview(&mut self, now: Moment) {
        if let Some(driver) = &mut self.preview.driver
            && let Err(cause) = driver.poll(now)
        {
            self.preview.record(PreviewHookCause::Driver(cause));
        }
        if self.preview.driver.as_ref().is_none_or(Preview::settled)
            && (self.preview.closing || self.preview.until.is_some_and(|until| now >= until))
        {
            self.preview.intent = None;
            self.preview.source = None;
            self.preview.rejected = None;
            let _passive = self.intent.take_preview();
        }
    }

    /// Cancel only existing work; actual completion/native closure still controls settlement.
    pub(in crate::production::child) fn close_preview(&mut self) {
        self.preview.closing = true;
        if let Some(driver) = &mut self.preview.driver {
            driver.close();
        }
    }

    /// Admit retirement only for an already requested current Main; early retry keeps its loan.
    pub(in crate::production::child) fn retire_preview_main(&mut self, identity: MainIdentity) {
        if let Some(driver) = &mut self.preview.driver {
            driver.retire_main(identity);
        }
    }

    /// A command that never entered the existing queue created no new native obligation.
    pub(in crate::production::child) fn reject_preview_command(&mut self) {
        self.preview.record(PreviewHookCause::Driver(PreviewError::State));
        self.close_preview();
    }

    /// Preserve the original clock cause and use only the frozen cutoff to veto further effects.
    pub(in crate::production::child) fn reject_preview_clock(&mut self, cause: ExecutionError) {
        self.preview.record(PreviewHookCause::Driver(PreviewError::Execution(cause)));
        self.close_preview();
        if let Some(until) = self.preview.until {
            self.poll_preview(until);
        }
    }

    /// Observe whether the same owner still needs its ordinary poll; inert modes sample nothing.
    pub(in crate::production::child) fn preview_needs_poll(&self) -> bool {
        self.preview_pending() || (self.preview.requested && self.preview.closing)
    }

    /// Veto cleanup while any real job, loan, input or uncertain file terminal remains owned.
    pub(in crate::production::child) fn preview_pending(&self) -> bool {
        self.preview.source.is_some()
            || self.preview.rejected.is_some()
            || self.preview.driver.as_ref().is_some_and(Preview::pending)
    }

    /// Borrow only the original campaign deadline while this qualifier still needs progress.
    pub(in crate::production::child) fn preview_deadline(&self) -> Option<Moment> {
        if self.preview_pending() { self.preview.until } else { None }
    }
}
impl PreviewHooks {
    fn record(&mut self, cause: PreviewHookCause) {
        if self.failure.is_none() {
            use sugarsubstitute_desktop::qualification_trace::Cause;
            let category = match &cause {
                PreviewHookCause::Capacity => Cause::Capacity,
                PreviewHookCause::Declaration(_) => Cause::Declaration,
                PreviewHookCause::Arming(_) => Cause::Arming,
                PreviewHookCause::Driver(error) => error.trace_cause(),
            };
            sugarsubstitute_desktop::qualification_trace::failure(None, category);
            tracing::warn!(operation="registered_main_preview_rejected", cause=%cause,
                "Original optional qualification cause retained; product startup policy unchanged");
            self.failure = Some(cause);
        }
    }
}
fn is_preview(observation: Option<&DesktopStartupObservation>) -> bool {
    matches!(
        observation,
        Some(
            DesktopStartupObservation::MainPreviewCaptured(_)
                | DesktopStartupObservation::MainPreviewFailed { .. }
        )
    )
}
fn preview_kind(observation: &DesktopStartupObservation) -> &'static str {
    match observation {
        DesktopStartupObservation::MainPreviewCaptured(_) => "captured",
        DesktopStartupObservation::MainPreviewFailed { .. } => "failed",
        _ => "ordinary",
    }
}
