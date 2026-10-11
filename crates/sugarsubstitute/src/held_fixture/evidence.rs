// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one held child's actual facts independently of requested cleanup effects.
use std::error::Error;
use sugarsubstitute_contracts::{
    execution::JobId,
    lifecycle::{ProcessKey, ShutdownId},
};
use sugarsubstitute_execution::{
    windows::NativeCorrelation,
    windows_fixture::{FixtureObservation, FixtureSessionReport},
};
use sugarsubstitute_supervision::{RegistryObservation, RootObservationOutcome};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Own bounded evidence for one isolated self-executable qualification case.
/// Identities do not grant native authority. The parent retains actual resources,
/// first failure and cleanup independently; rejected evidence cannot stop cleanup.
pub(super) struct Evidence {
    pub(super) expected: u32,
    pub(super) correlation: NativeCorrelation,
    pub(super) process: Option<ProcessKey>,
    pub(super) root_job: Option<JobId>,
    pub(super) prepared: bool,
    pub(super) ready: bool,
    pub(super) cancel_requested: Option<JobId>,
    pub(super) retirement: Option<JobId>,
    pub(super) expected_failure: bool,
    pub(super) challenge_requested: bool,
    pub(super) responded: bool,
    pub(super) release_requested: bool,
    pub(super) exited: Option<u32>,
    pub(super) shutdown: Option<ShutdownId>,
    pub(super) settled: bool,
    pub(super) retired: bool,
    pub(super) session_report: Option<FixtureSessionReport>,
}
impl Evidence {
    /// Derive a positive, nonwrapping correlation from the zero-based case index.
    pub(super) fn new(index: u64, expected: u32) -> Result<Self> {
        let correlation =
            NativeCorrelation::new(index.checked_add(1).ok_or("Held correlation exhausted")?)?;
        Ok(Self {
            expected,
            correlation,
            process: None,
            root_job: None,
            prepared: false,
            ready: false,
            cancel_requested: None,
            retirement: None,
            expected_failure: false,
            challenge_requested: false,
            responded: false,
            release_requested: false,
            exited: None,
            shutdown: None,
            settled: false,
            retired: false,
            session_report: None,
        })
    }
    /// Bind the admitted family and its original root-watch job exactly once.
    pub(super) fn bind(&mut self, process: ProcessKey, root_job: JobId) -> Result<()> {
        if self.process.is_some() || self.root_job.is_some() {
            return Err("Held evidence already has a family/root watch".into());
        }
        self.process = Some(process);
        self.root_job = Some(root_job);
        Ok(())
    }
    /// Consume actual protocol milestones in order; requests never imply responses.
    pub(super) fn protocol(&mut self, observation: FixtureObservation) -> Result<()> {
        match observation {
            FixtureObservation::Prepared if !self.prepared => self.prepared = true,
            FixtureObservation::Ready if self.prepared && self.process.is_some() && !self.ready => {
                self.ready = true
            }
            FixtureObservation::Responded
                if self.ready
                    && self.challenge_requested
                    && self.cancel_retired()
                    && !self.responded =>
            {
                self.responded = true
            }
            _ => return Err("Held protocol evidence is duplicate or premature".into()),
        }
        Ok(())
    }
    /// Record one exact root-watch cancellation request without claiming retirement.
    pub(super) fn request_cancel(&mut self, job: JobId) -> Result<()> {
        if self.root_job != Some(job) || self.cancel_requested.is_some() {
            return Err("Held cancellation does not identify the one original root watch".into());
        }
        self.cancel_requested = Some(job);
        Ok(())
    }
    /// Require Ready and actual Cancelled retirement before requesting Challenge.
    pub(super) fn request_challenge(&mut self) -> Result<()> {
        if !self.ready
            || !self.cancel_retired()
            || self.challenge_requested
            || self.release_requested
        {
            return Err(
                "Held Challenge lacks Ready/Cancelled retirement or admission is closed".into()
            );
        }
        self.challenge_requested = true;
        Ok(())
    }
    /// Record Release once after actual Ready, including failure cleanup before Response.
    pub(super) fn request_release(&mut self) -> Result<()> {
        if !self.ready || self.release_requested {
            return Err("Held Release lacks Ready or was already requested".into());
        }
        self.release_requested = true;
        Ok(())
    }
    /// Retain the original stop identity exactly once; it does not prove settlement.
    pub(super) fn begin_stop(&mut self, shutdown: ShutdownId) -> Result<()> {
        if self.process.is_none() || self.shutdown.is_some() {
            return Err("Held stop lacks a bound family or was already requested".into());
        }
        self.shutdown = Some(shutdown);
        Ok(())
    }
    /// Consume exact registry observations while keeping actual exit facts on failure.
    /// Only the plain expected cancellation error is accepted; every other failure
    /// returns its original owned source. Native cleanup and fatal facts stay visible.
    pub(super) fn observe(&mut self, observation: RegistryObservation) -> Result<()> {
        let process = match &observation {
            RegistryObservation::ContainedStarted { process }
            | RegistryObservation::RootExited { process, .. }
            | RegistryObservation::RootObservationRetired { process, .. }
            | RegistryObservation::FamilySettled { process, .. }
            | RegistryObservation::Failed { process, .. } => *process,
        };
        if self.process != Some(process) || self.retired {
            if let RegistryObservation::Failed { error, .. } = observation {
                return Err(Box::new(error));
            }
            return Err("Held registry observation has a foreign or retired identity".into());
        }
        match observation {
            RegistryObservation::ContainedStarted { .. } => {}
            RegistryObservation::RootObservationRetired { job, outcome, .. } => {
                if self.retirement.is_some()
                    || self.cancel_requested != Some(job)
                    || self.root_job != Some(job)
                    || outcome != RootObservationOutcome::Cancelled
                {
                    return Err(
                        "Held root retirement is not the exact requested Cancelled watch".into()
                    );
                }
                self.retirement = Some(job);
            }
            RegistryObservation::RootExited { code, .. } => {
                if self.exited.is_some() {
                    return Err("Held root exit was already observed".into());
                }
                self.exited = Some(code);
                if code != self.expected || !self.release_requested || !self.responded {
                    return Err("Held root actually exited early or with an unexpected code".into());
                }
            }
            RegistryObservation::FamilySettled { shutdown, .. } => {
                if self.settled || self.shutdown.is_none() || shutdown != self.shutdown {
                    return Err("Held family settlement lacks the exact original stop".into());
                }
                self.settled = true;
            }
            RegistryObservation::Failed { error, .. } => {
                if !self.accept_expected_failure(
                    error.stage(),
                    error.cleanup_causes().len(),
                    error.omitted_causes(),
                    error.contains_owner_panic(),
                ) {
                    return Err(Box::new(error));
                }
            }
        }
        Ok(())
    }
    /// Retain the first matching terminal report before rejecting incomplete proof.
    /// The report cannot manufacture earlier actual milestones or process settlement.
    pub(super) fn report(&mut self, report: FixtureSessionReport) -> Result<()> {
        if report.correlation != self.correlation || self.session_report.is_some() {
            return Err("Held session report is foreign or duplicated".into());
        }
        self.session_report = Some(report);
        if !self.protocol_complete() || !report_complete(report) {
            return Err("Held session report lacks actual preceding protocol evidence".into());
        }
        Ok(())
    }
    fn cancel_retired(&self) -> bool {
        self.root_job.is_some()
            && self.retirement == self.root_job
            && self.cancel_requested == self.root_job
    }
    fn accept_expected_failure(
        &mut self,
        stage: &str,
        cleanup_count: usize,
        omitted: u64,
        owner_panic: bool,
    ) -> bool {
        if self.cancel_retired()
            && !self.expected_failure
            && expected_failure(stage, cleanup_count, omitted, owner_panic)
        {
            self.expected_failure = true;
            true
        } else {
            false
        }
    }
    fn protocol_complete(&self) -> bool {
        self.prepared
            && self.ready
            && self.cancel_retired()
            && self.challenge_requested
            && self.responded
            && self.release_requested
    }
    /// Require every actual milestone; the parent separately requires no qualification failure.
    /// `retired` is set only after the parent successfully calls registry.retire_settled.
    pub(super) fn complete(&self) -> bool {
        self.process.is_some()
            && self.protocol_complete()
            && self.expected_failure
            && self.exited == Some(self.expected)
            && self.shutdown.is_some()
            && self.settled
            && self.retired
            && self.session_report.is_some_and(|report| {
                report.correlation == self.correlation && report_complete(report)
            })
    }
}
fn report_complete(report: FixtureSessionReport) -> bool {
    report.ready && report.challenge_sent && report.responded && report.release_sent
}
/// The expected typed cancellation creates one specific plain registry error.
/// Any native cleanup, omitted cause or sticky owner panic changes qualification.
fn expected_failure(stage: &str, cleanup_count: usize, omitted: u64, owner_panic: bool) -> bool {
    stage == "process_observation_not_exit" && cleanup_count == 0 && omitted == 0 && !owner_panic
}

#[cfg(test)]
#[path = "evidence_tests.rs"]
mod tests;
