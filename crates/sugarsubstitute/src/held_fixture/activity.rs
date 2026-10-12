// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound campaign decisions and require actual cancellation before live child challenge.
use super::*;
use sugarsubstitute_supervision::RegistryObservation;

impl Fixture {
    /// Drain the bounded completion batch and visit at most five retained cases.
    pub(super) fn advance(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        let mut progress = self.drain(max)?;
        let external = self.external_wake.as_ref().is_some_and(|wake| wake.take_changed());
        let boundary_due = self
            .cleanup
            .iter()
            .filter_map(|cleanup| cleanup.stop.as_ref().and_then(StopPlan::next_deadline))
            .any(|deadline| Instant::now() >= deadline);
        self.scan.begin(external, boundary_due);
        if !self.deadline_observed && Instant::now() >= self.deadline {
            self.deadline_observed = true;
            return Err("Held campaign exceeded its original60-second deadline".into());
        }
        if self.registry.is_none() {
            return Ok(if self.closing { HeadlessStep::Shutdown } else { HeadlessStep::Active });
        }
        // Five cases transfer at most two facts, one terminal and two decisions
        // each. Sentinel and global advancement keep the total below32.
        for _ in 0..5 {
            if !self.scan.visit() {
                break;
            }
            let index = self.cursor;
            self.cursor = (self.cursor + 1) % HELD;
            match self.case_activity(index) {
                Ok(changed) => progress |= changed,
                Err(error) => {
                    self.scan.retry_after_error();
                    if let Err(cause) = self.notify() {
                        tracing::error!(operation = "held_campaign_error_wake", cause = %cause,
                            "Campaign retains its original failure while continuation wake failed");
                    }
                    return Err(error);
                }
            }
        }
        if let Some(key) = self.sentinel.process
            && !self.sentinel.retired
        {
            if let Some(observation) = self
                .registry
                .as_mut()
                .ok_or("Actual registry is not prepared")?
                .take_observation(key)?
            {
                if matches!(&observation, RegistryObservation::FamilySettled { process, .. } if *process == key)
                {
                    self.sentinel_settlement_observed = true;
                }
                self.sentinel.observe(observation)?;
                progress = true;
            }
            if self.sentinel.exited.is_some() || self.closing {
                progress |= self.begin_stop(SENTINEL)?;
            }
            progress |= self.retire_or_expire(SENTINEL)?;
        }
        if !self.closing {
            let phase_progress = self.phase_activity()?;
            if phase_progress {
                self.scan.request();
            }
            progress |= phase_progress;
        }
        progress |= self.cleanup_step()?;
        if progress || self.scan.pending() {
            self.notify()?;
        }
        let done = self.sentinel.complete()
            && self.cases.iter().all(|case| case.started && case.evidence.complete());
        Ok(if self.closing || done { HeadlessStep::Shutdown } else { HeadlessStep::Active })
    }
    /// Advance causal protocol phases using cached actual observations only.
    fn phase_activity(&mut self) -> Result<bool> {
        match self.phase {
            Phase::Enrolling if self.enrolling == HELD => {
                self.launch_sentinel()?;
                self.phase = Phase::Sentinel;
                Ok(true)
            }
            Phase::Enrolling => self.enroll(),
            Phase::Sentinel if self.sentinel.started && self.sentinel.exited == Some(259) => {
                self.phase = Phase::Cancelling;
                Ok(true)
            }
            Phase::Cancelling if self.cases.iter().all(|case| case.evidence.responded) => {
                self.phase = Phase::Releasing;
                Ok(true)
            }
            Phase::Releasing if self.cases.iter().all(|case| case.evidence.release_requested) => {
                self.phase = Phase::Collecting;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    /// Transfer fixed facts without allowing rejected qualification to block cleanup.
    fn case_activity(&mut self, index: usize) -> Result<bool> {
        let mut progress = false;
        if let Some(session) = &self.cases[index].session
            && let Some(observation) = session.control.take_observation()
        {
            self.cases[index].evidence.protocol(observation)?;
            progress = true;
        }
        if let Some(key) = self.cases[index].evidence.process
            && !self.cases[index].evidence.retired
            && let Some(observation) = self
                .registry
                .as_mut()
                .ok_or("Actual registry is not prepared")?
                .take_observation(key)?
        {
            if matches!(&observation, RegistryObservation::ContainedStarted { process } if *process == key)
            {
                self.cases[index].started = true;
            }
            if matches!(&observation, RegistryObservation::FamilySettled { process, .. } if *process == key)
            {
                self.cases[index].settlement_observed = true;
            }
            self.cases[index].evidence.observe(observation)?;
            progress = true;
        }
        progress |= self.event_terminal(index)?;
        progress |= self.case_command(index)?;
        progress |= self.retire_or_expire(index)?;
        Ok(progress)
    }
    /// Separate watch cancellation, live challenge and explicit process release.
    fn case_command(&mut self, index: usize) -> Result<bool> {
        if self.closing {
            if !self.cases[index].closing_action_done {
                self.cases[index].closing_action_done = true;
                if let Some(session) = &self.cases[index].session {
                    if self.cases[index].evidence.ready
                        && !self.cases[index].evidence.release_requested
                    {
                        session.control.request_release()?;
                        self.cases[index].evidence.request_release()?;
                    } else if !self.cases[index].evidence.release_requested {
                        session.control.cancel();
                    }
                }
                return Ok(true);
            }
            return self.begin_stop(index);
        }
        let evidence = &self.cases[index].evidence;
        match self.phase {
            Phase::Cancelling if evidence.cancel_requested.is_none() => {
                let key = evidence.process.ok_or("Held cancellation lacks family")?;
                let job = self
                    .registry
                    .as_mut()
                    .ok_or("Actual held registry is not prepared")?
                    .request_observation_retirement(key)?;
                self.cases[index].evidence.request_cancel(job)?;
                Ok(true)
            }
            Phase::Cancelling if evidence.retirement.is_some() && !evidence.challenge_requested => {
                self.cases[index]
                    .session
                    .as_ref()
                    .ok_or("Held challenge lacks session")?
                    .control
                    .request_challenge()?;
                self.cases[index].evidence.request_challenge()?;
                Ok(true)
            }
            Phase::Releasing if !evidence.release_requested => {
                self.cases[index]
                    .session
                    .as_ref()
                    .ok_or("Held release lacks session")?
                    .control
                    .request_release()?;
                self.cases[index].evidence.request_release()?;
                Ok(true)
            }
            Phase::Releasing | Phase::Collecting if evidence.release_requested => {
                self.begin_stop(index)
            }
            _ => Ok(false),
        }
    }
}
