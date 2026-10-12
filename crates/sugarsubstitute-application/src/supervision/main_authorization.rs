// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Sequence inert main-attempt facts without constructing native authorization.
//!
//! The composing host authenticates requests and validates retained profile/browser
//! proofs before submitting observations. Effects request that host's separately
//! checked operations. Retirement confirmation additionally requires actual native
//! retirement and consumption of every finite proof job; this owner performs no IO.

mod error;
mod history;
#[cfg(test)]
mod tests;
mod transition;
mod types;

pub use error::{MainAuthorizationError, MainAuthorizationRejected};
use history::{Fingerprint, RequestHistory};
use std::num::NonZeroU32;
use sugarsubstitute_contracts::{
    execution::{Moment, TimeError},
    lifecycle::{ClientBinding, LeaseKey, LifecycleFailure, LifecycleRejection, RequestId, RunKey},
    startup::MainIdentity,
};
pub use types::{
    MainAuthorizationEffect, MainAuthorizationObservation, MainAuthorizationSnapshot,
    MainAuthorizationStage,
};

/// Maximum retained exact request fingerprints; evicted IDs never execute again.
pub const MAIN_AUTHORIZATION_HISTORY: usize = 8;

/// Own one run's current main sequence, original deadline and monotonic retirement.
///
/// These fields establish application ordering only. They authenticate no native
/// peer, resource, installed file, environment, browser family or retirement.
pub struct MainAuthorizationSession {
    run: RunKey,
    identity: Option<MainIdentity>,
    lease: Option<LeaseKey>,
    until: Option<Moment>,
    stage: MainAuthorizationStage,
    history: RequestHistory,
    last_now: Option<Moment>,
    folder_offset: u32,
    environment_request: Option<RequestId>,
    browser_request: Option<RequestId>,
    browser_pid: Option<NonZeroU32>,
    retirement_observed: bool,
    failure_seen: bool,
    failure: Option<LifecycleFailure>,
}

impl MainAuthorizationSession {
    /// Create an empty, retired session for one exact coordinator-bound run.
    pub fn new(run: RunKey) -> Self {
        Self {
            run,
            identity: None,
            lease: None,
            until: None,
            stage: MainAuthorizationStage::Retired,
            history: RequestHistory::default(),
            last_now: None,
            folder_offset: 0,
            environment_request: None,
            browser_request: None,
            browser_pid: None,
            retirement_observed: false,
            failure_seen: false,
            failure: None,
        }
    }

    /// Begin a fully retired successor using the host's original frontend deadline.
    ///
    /// The full lease must match this run and coordinator. Both launch and
    /// navigation increase; request high-water and observed clock order persist.
    /// # Errors
    /// Rejects wrong bindings, unsettled predecessors, untransferred failure causes
    /// or non-increasing attempts before changing state. No native trust is granted.
    pub fn begin(
        &mut self,
        identity: MainIdentity,
        lease: LeaseKey,
        frontend_until: Moment,
    ) -> Result<(), LifecycleRejection> {
        if identity.run != self.run || lease.run != self.run {
            return Err(LifecycleRejection::StaleIdentity);
        }
        if lease.resource.owner != self.run.owner {
            return Err(LifecycleRejection::ForeignOwner);
        }
        if self.stage != MainAuthorizationStage::Retired || self.failure.is_some() {
            return Err(LifecycleRejection::WrongPhase);
        }
        if self.identity.is_some_and(|old| {
            identity.launch.get() <= old.launch.get()
                || identity.navigation.get() <= old.navigation.get()
        }) {
            return Err(LifecycleRejection::StaleIdentity);
        }
        self.identity = Some(identity);
        self.lease = Some(lease);
        self.until = Some(frontend_until);
        self.stage = MainAuthorizationStage::RequestedFolder;
        self.history.clear_records();
        self.folder_offset = 0;
        self.environment_request = None;
        self.browser_request = None;
        self.browser_pid = None;
        self.retirement_observed = false;
        self.failure_seen = false;
        Ok(())
    }

    /// Consume an authenticated request or its host-validated completion/issue fact.
    ///
    /// Completion variants continue the original environment/controller request,
    /// using its actual ID rather than inventing another request. Exact duplicates
    /// emit no effects and never renew the deadline. `now` uses the same injected
    /// monotonic epoch as `frontend_until`, in nanoseconds.
    /// # Errors
    /// Returns the unchanged observation, including owned cause, on wrong binding,
    /// stage, duplicate, expired ordinary work or clock regression. Retirement and
    /// failure can arrive after frontend expiry; they still check clock ordering.
    pub fn accept(
        &mut self,
        binding: ClientBinding,
        request: RequestId,
        observation: MainAuthorizationObservation,
        now: Moment,
    ) -> Result<Option<MainAuthorizationEffect>, MainAuthorizationRejected> {
        match self.check(binding, &observation, now) {
            Ok(()) => {}
            Err(error) => return Err(MainAuthorizationRejected { error, observation }),
        }
        match self.transition(request, &observation) {
            Ok(effect) => {
                self.last_now = Some(now);
                if let MainAuthorizationObservation::Failed { failure, .. } = observation {
                    self.failure = Some(failure);
                    self.failure_seen = true;
                }
                Ok(effect)
            }
            Err(error) => Err(MainAuthorizationRejected { error, observation }),
        }
    }

    fn check(
        &self,
        binding: ClientBinding,
        observation: &MainAuthorizationObservation,
        now: Moment,
    ) -> Result<(), MainAuthorizationError> {
        if binding.run != self.run
            || self.identity != Some(observation.identity())
            || self.lease.is_none_or(|lease| lease.connection != binding.connection)
        {
            return Err(LifecycleRejection::StaleIdentity.into());
        }
        if self.last_now.is_some_and(|previous| now < previous) {
            return Err(MainAuthorizationError::Clock(TimeError::Regression));
        }
        if !observation.is_retirement_or_failure() {
            if matches!(
                self.stage,
                MainAuthorizationStage::Retiring
                    | MainAuthorizationStage::Retired
                    | MainAuthorizationStage::Failed
            ) {
                return Err(LifecycleRejection::WrongPhase.into());
            }
            if self.until.is_none_or(|until| now >= until) {
                return Err(MainAuthorizationError::Deadline);
            }
        }
        Ok(())
    }

    /// Close ordinary/proof admission immediately, before native cleanup admission.
    /// # Errors
    /// Rejects an unrelated identity or an already retired attempt. Repeated
    /// retirement is idempotent; rejected/failed native close cannot reopen it.
    pub fn retire(&mut self, identity: MainIdentity) -> Result<(), LifecycleRejection> {
        if self.identity != Some(identity) {
            return Err(LifecycleRejection::StaleIdentity);
        }
        if self.stage == MainAuthorizationStage::Retired {
            return Err(LifecycleRejection::WrongPhase);
        }
        self.stage = MainAuthorizationStage::Retiring;
        Ok(())
    }

    /// Confirm the host's actual native retirement and consumed finite proof jobs.
    ///
    /// Caller must join those actual adapter facts before calling. A request,
    /// deadline, dropped result or failed native close is insufficient.
    /// # Errors
    /// Rejects mismatched identity, missing exact `MainRetired` observation or wrong
    /// phase. Success permits a strictly newer attempt, retaining original failure.
    pub fn confirm_retired(&mut self, identity: MainIdentity) -> Result<(), LifecycleRejection> {
        if self.identity != Some(identity) {
            return Err(LifecycleRejection::StaleIdentity);
        }
        if self.stage != MainAuthorizationStage::Retiring || !self.retirement_observed {
            return Err(LifecycleRejection::WrongPhase);
        }
        self.stage = MainAuthorizationStage::Retired;
        Ok(())
    }

    /// Observe inert ordering state; it is not a native authorization decision.
    pub const fn stage(&self) -> MainAuthorizationStage {
        self.stage
    }

    /// Copy bounded current facts without IO, allocation or native access.
    pub fn snapshot(&self) -> MainAuthorizationSnapshot {
        MainAuthorizationSnapshot {
            identity: self.identity,
            lease: self.lease,
            frontend_until: self.until,
            stage: self.stage,
            requested_offset_units: self.folder_offset,
            browser_process_id: self.browser_pid,
            retirement_observed: self.retirement_observed,
            failure_code: self.failure.as_ref().map(LifecycleFailure::code),
            failure_observed: self.failure_seen,
            request_highwater: self.history.highwater(),
        }
    }

    /// Borrow the first owned failure with its original source chain intact.
    pub fn failure(&self) -> Option<&LifecycleFailure> {
        self.failure.as_ref()
    }

    /// Transfer the first cause without reopening its failure/admission fence.
    pub fn take_failure(&mut self) -> Option<LifecycleFailure> {
        self.failure.take()
    }
}
