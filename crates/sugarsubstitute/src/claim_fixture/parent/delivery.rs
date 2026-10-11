// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match terminal deliveries and event facts to their original admitted owners.
use super::*;
use sugarsubstitute_contracts::execution::JobOutcome;
impl Fixture {
    pub(super) fn deliver(&mut self) -> Result<()> {
        if self.setup_notice
            && let Some(completion) = self.setup.as_mut().and_then(OwnedJob::try_take_completion)
        {
            let job = self.setup.take().ok_or("Claim artifact job disappeared")?;
            if (completion.scope_id, completion.spec) != (job.scope_id(), job.spec()) {
                return Err("Claim setup terminal identity changed".into());
            }
            self.setup_notice = false;
            failure::terminal(completion.outcome)?;
            self.setup_finished = true;
        }
        let Some(case) = &mut self.child else { return Ok(()) };
        if let Some(session) = &case.session {
            for _ in 0..3 {
                let Some(event) = session.control.take_observation() else { break };
                match event {
                    FixtureObservation::Prepared => case.prepared = true,
                    FixtureObservation::Ready => {
                        if !case.prepared || case.family.is_none() {
                            return Err("Claim child ready before contained enrollment".into());
                        }
                        case.ready = true;
                    }
                    FixtureObservation::Responded => {
                        if !case.ready || !case.challenge_sent {
                            return Err("Claim child response lacks parent challenge".into());
                        }
                        case.responded = true;
                    }
                }
            }
        }
        if let Some(kind) = case.event_notice
            && let Some(completion) =
                case.session.as_mut().and_then(|s| s.completion.try_take_completion())
        {
            let session = case.session.take().ok_or("Claim parent session disappeared")?;
            if (completion.scope_id, completion.spec)
                != (session.completion.scope_id(), session.completion.spec())
            {
                return Err("Claim event terminal identity changed".into());
            }
            case.event_notice = None;
            case.event_retired = true;
            match completion.outcome {
                JobOutcome::Success(report) => {
                    if report.correlation != NativeCorrelation::new(case.index as u64 + 1)? {
                        return Err("Claim event correlation changed".into());
                    }
                    case.event_report = Some(report);
                    if !self.closing
                        && (case.abortive
                            || !report.ready
                            || !report.challenge_sent
                            || !report.responded
                            || !report.release_sent)
                    {
                        return Err("Claim child normal event protocol is incomplete".into());
                    }
                }
                JobOutcome::Cancelled
                    if case.event_cancel_requested && (case.abortive || self.closing) =>
                {
                    if kind != TerminalKind::Cancelled || (case.abortive && case.release_sent) {
                        return Err("Claim holder cancellation fabricated release".into());
                    }
                }
                outcome => {
                    failure::terminal(outcome)?;
                }
            }
        }
        if case.read_notice
            && let Some(completion) = case.read.as_mut().and_then(OwnedJob::try_take_completion)
        {
            let job = case.read.take().ok_or("Claim child read disappeared")?;
            if (completion.scope_id, completion.spec) != (job.scope_id(), job.spec()) {
                return Err("Claim child diagnostic identity changed".into());
            }
            case.read_notice = false;
            let report = failure::terminal(completion.outcome)?;
            report.validate(
                case.role,
                case.index as u64 + 1,
                self.child_deadline.ticks(),
                self.child_deadline.frequency(),
            )?;
            case.receipt = Some(report);
        }
        Ok(())
    }
    pub(super) fn drive_child(&mut self) -> Result<bool> {
        let Some(case) = &mut self.child else { return Ok(false) };
        let mut changed = false;
        if let Some(session) = &case.session {
            if case.ready && !case.challenge_sent {
                session.control.request_challenge()?;
                case.challenge_sent = true;
                changed = true;
            }
            if case.responded && !case.abortive && !case.release_sent {
                session.control.request_release()?;
                case.release_sent = true;
                changed = true;
            }
        }
        if case.family.as_ref().is_some_and(|f| f.progress.exited.is_some()) {
            changed |= self.begin_stop(false)?;
            changed |= self.read_child_receipt()?;
        }
        changed |= self.family_step(false, true)?;
        changed |= self.retire_family(false)?;
        Ok(changed)
    }
    pub(super) fn finish_child(&mut self) -> Result<bool> {
        let Some(case) = &self.child else { return Ok(false) };
        if !case.event_retired
            || case.read.is_some()
            || case.family.as_ref().is_none_or(|f| !f.progress.complete())
            || (!case.abortive && case.receipt.is_none())
        {
            return Ok(false);
        }
        let case = self.child.take().ok_or("Claim child disappeared")?;
        let family = case.family.ok_or("Claim child family missing")?;
        let budget = family.stop.as_ref().ok_or("Claim child lacks original StopPlan")?.budget;
        let shutdown = family.progress.stop.ok_or("Claim child lacks exact shutdown identity")?.0;
        if self.results.len() != case.index {
            return Err("Claim child result order changed".into());
        }
        self.results.push(serde_json::json!({
            "role":case.role.wire(),"correlation":case.index as u64+1,
            "process_id":family.ticket.process().id.get(),
            "creation_job":family.creation.get(),"root_watch_job":family.root_watch.get(),
            "shutdown_id":shutdown.get(),"shutdown_budget_nanoseconds":{"started":budget.started.as_nanos(),"graceful_until":budget.graceful_until.as_nanos(),"settle_until":budget.settle_until.as_nanos()},
            "contained_started":family.progress.started,"actual_root_exit":family.progress.exited,"family_settled":family.progress.settled,"registry_retired":family.progress.retired,
            "ready_observed":case.ready,"challenge_requested":case.challenge_sent,"response_observed":case.responded,
            "release_requested":case.release_sent,"event_cancel_requested":case.event_cancel_requested,"event_retired":case.event_retired,
            "diagnostic_child_receipt":case.receipt,
        }));
        Ok(true)
    }
}
