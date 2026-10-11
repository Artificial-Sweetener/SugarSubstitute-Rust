// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain independent actual pipe, process and family facts for three fixed cases.
use super::{Result, options::CaseKind};
use std::time::Instant;
use sugarsubstitute_contracts::{
    execution::{JobId, Moment},
    lifecycle::{ProcessKey, RunKey, ShutdownBudget, ShutdownId},
};
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    LifecyclePipeDeadline, LifecyclePipeFailure, LifecyclePipeSession, VerifiedLifecyclePeer,
};
use sugarsubstitute_supervision::RegistryObservation;

pub(super) struct Case {
    pub(super) kind: CaseKind,
    pub(super) run: RunKey,
    pub(super) process: Option<ProcessKey>,
    pub(super) creation_job: Option<JobId>,
    pub(super) session: Option<LifecyclePipeSession>,
    pub(super) bootstrap_deadline: Option<LifecyclePipeDeadline>,
    pub(super) prepared: bool,
    pub(super) connected: bool,
    pub(super) hello: bool,
    pub(super) auth_requested: bool,
    pub(super) peer: Option<VerifiedLifecyclePeer>,
    pub(super) welcome_sent: bool,
    pub(super) welcome_written: bool,
    pub(super) register: bool,
    pub(super) reply_sent: bool,
    pub(super) reply_written: bool,
    pub(super) denied: bool,
    pub(super) expected_failure: Option<LifecyclePipeFailure>,
    pub(super) pipe_close_requested: bool,
    pub(super) pipe_terminal: bool,
    pub(super) terminal_notice: super::terminal::PendingTerminal,
    pub(super) started: bool,
    pub(super) exited: Option<u32>,
    pub(super) stop: Option<Stop>,
    pub(super) settled: bool,
    settlement_observed: bool,
    pub(super) retired: bool,
    pub(super) cleanup_requested: bool,
    pub(super) cleanup_job: Option<JobId>,
    pub(super) member_job: Option<JobId>,
}
impl Case {
    pub(super) fn new(kind: CaseKind, run: RunKey) -> Self {
        Self {
            kind,
            run,
            process: None,
            creation_job: None,
            session: None,
            bootstrap_deadline: None,
            prepared: false,
            connected: false,
            hello: false,
            auth_requested: false,
            peer: None,
            welcome_sent: false,
            welcome_written: false,
            register: false,
            reply_sent: false,
            reply_written: false,
            denied: false,
            expected_failure: None,
            pipe_close_requested: false,
            pipe_terminal: false,
            terminal_notice: Default::default(),
            started: false,
            exited: None,
            stop: None,
            settled: false,
            settlement_observed: false,
            retired: false,
            cleanup_requested: false,
            cleanup_job: None,
            member_job: None,
        }
    }
    /// Record matching exit facts before rejecting their status; cleanup still needs them.
    pub(super) fn observe_registry(&mut self, observation: RegistryObservation) -> Result<()> {
        let observed = match &observation {
            RegistryObservation::ContainedStarted { process }
            | RegistryObservation::RootExited { process, .. }
            | RegistryObservation::RootObservationRetired { process, .. }
            | RegistryObservation::FamilySettled { process, .. }
            | RegistryObservation::Failed { process, .. } => *process,
        };
        if Some(observed) != self.process || self.retired {
            return Err("Lifecycle family observation has foreign/retired identity".into());
        }
        match observation {
            RegistryObservation::ContainedStarted { .. } => {
                if self.started {
                    return Err("Lifecycle family startup repeated".into());
                }
                self.started = true;
            }
            RegistryObservation::RootExited { code, .. } => {
                if self.exited.is_some() {
                    return Err("Lifecycle root exit repeated".into());
                }
                self.exited = Some(code);
                if code != self.kind.exit_code() {
                    return Err("Lifecycle child actual status differs from required role".into());
                }
            }
            RegistryObservation::FamilySettled { shutdown, .. } => {
                // Actual matching retirement permits cleanup even when required
                // qualification ordering failed. It does not repair that failure.
                self.settlement_observed = true;
                if self.settled
                    || shutdown != self.stop.as_ref().map(|stop| stop.id)
                    || shutdown.is_none()
                {
                    return Err(
                        "Lifecycle settlement differs from original stop transaction".into()
                    );
                }
                self.settled = true;
            }
            RegistryObservation::RootObservationRetired { .. } => {
                return Err(
                    "Lifecycle child requires actual root exit, not cancelled observation".into()
                );
            }
            RegistryObservation::Failed { error, .. } => {
                tracing::error!(operation="lifecycle_fixture_family", registry_process_id=observed.id.get(), stage=error.stage(), cause=?std::error::Error::source(&error), owner_panic=error.contains_owner_panic(), "Retained lifecycle family failure");
                return Err(Box::new(error));
            }
        }
        Ok(())
    }
    /// Retire failed cases only after an independent actual registry settlement.
    pub(super) fn can_retire(&self, closing: bool, actual_settled: bool) -> bool {
        !self.retired && actual_settled && (self.settled || closing && self.settlement_observed)
    }
    pub(super) fn complete(&self) -> bool {
        let protocol = match self.kind {
            CaseKind::Valid => {
                self.peer.is_some() && self.register && self.reply_written && !self.denied
            }
            _ => self.denied && self.peer.is_none() && !self.register && !self.reply_sent,
        };
        protocol
            && self.prepared
            && self.connected
            && self.hello
            && self.pipe_terminal
            && self.started
            && self.exited == Some(self.kind.exit_code())
            && self.settled
            && self.retired
    }
}
pub(super) struct Stop {
    pub(super) id: ShutdownId,
    pub(super) budget: ShutdownBudget,
    graceful: Instant,
    total: Instant,
    grace_observed: bool,
    total_observed: bool,
}
impl Stop {
    pub(super) fn new(id: ShutdownId, now: Moment, native_now: Instant) -> Result<Self> {
        let budget = ShutdownBudget::new(now)?;
        Ok(Self {
            id,
            budget,
            graceful: native_now
                .checked_add(budget.graceful_until.checked_duration_since(now)?)
                .ok_or("Lifecycle grace overflow")?,
            total: native_now
                .checked_add(budget.settle_until.checked_duration_since(now)?)
                .ok_or("Lifecycle total overflow")?,
            grace_observed: false,
            total_observed: false,
        })
    }
    /// Consume each original boundary once; an expired live family remains owned.
    pub(super) fn due(&mut self, now: Moment, settled: bool) -> (bool, bool) {
        let grace = !self.grace_observed && now >= self.budget.graceful_until;
        let total = !self.total_observed && now >= self.budget.settle_until;
        self.grace_observed |= grace;
        self.total_observed |= total;
        (grace || total, total && !settled)
    }
    pub(super) fn next_deadline(&self) -> Option<Instant> {
        if !self.grace_observed {
            Some(self.graceful)
        } else if !self.total_observed {
            Some(self.total)
        } else {
            None
        }
    }
}
