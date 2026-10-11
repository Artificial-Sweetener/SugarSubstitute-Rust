// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Advance one bounded campaign only from actual activity and original budget boundaries.
use super::*;
use sugarsubstitute_execution::windows_fixture::FixtureObservation;
impl Fixture {
    pub(super) fn advance(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        let batch = self.client.as_ref().ok_or("Descendant client")?.drain_completions(max)?;
        self.registry.consume(&batch)?;
        for notice in &batch.notifications {
            if Some(notice.job_id) == self.cleanup_job {
                self.cleanup_job = None;
                if let Some(key) = self.key {
                    let snapshot = self.registry.snapshot(key)?;
                    self.cleanup_needed |= notice.kind == TerminalKind::Success
                        && snapshot.member_watch_required
                        && !snapshot.cleanup_pending
                        && !snapshot.settled;
                }
            }
            if Some(notice.job_id) == self.member_job {
                self.member_job = None;
                self.member_notice = Some(notice.kind);
                self.cleanup_needed = true;
            }
        }
        if let Some(key) = self.key {
            let snapshot = self.registry.snapshot(key)?;
            self.evidence.forced |= snapshot.forced;
            self.evidence.retained = self.evidence.retained.max(snapshot.retained_members);
            if let Some(exit) = snapshot.last_member_exit {
                self.descendant_exit = Some(exit);
                self.evidence.observe_member(
                    exit,
                    key.id.get(),
                    self.member_notice.take(),
                    self.closing,
                )?;
            }
            for _ in 0..max.get() {
                let Some(observation) = self.registry.take_observation(key)? else { break };
                match observation {
                    RegistryObservation::ContainedStarted { process } if process == key => {
                        self.evidence.started = true
                    }
                    RegistryObservation::RootExited { process, code } if process == key => {
                        self.evidence.root_exit(code)?;
                        self.begin_stop()?;
                    }
                    RegistryObservation::FamilySettled { process, shutdown }
                        if process == key && shutdown == Some(ShutdownId::new(1)?) =>
                    {
                        self.evidence.settlement()?
                    }
                    RegistryObservation::Failed { error, .. } => return Err(Box::new(error)),
                    _ => return Err("Unexpected descendant family observation".into()),
                }
            }
            let snapshot = self.registry.snapshot(key)?;
            self.evidence.retained = self.evidence.retained.max(snapshot.retained_members);
            if let Some(stop) = &mut self.stop {
                let now = self.clock.now();
                let due = stop.due(now, snapshot.settled);
                if now >= stop.budget.graceful_until {
                    self.evidence.grace = true;
                }
                self.cleanup_needed |= due.cleanup;
                if due.expired {
                    return Err(
                        "Descendant family exceeded original15-second settlement budget".into()
                    );
                }
            }
            if self.evidence.can_retire_family(snapshot.settled, self.closing) {
                self.registry.retire_settled(key)?;
                self.key = None;
                self.evidence.retired = true;
            }
        }
        if let Some(session) = &self.session {
            for _ in 0..3 {
                let Some(observed) = session.control.take_observation() else { break };
                match observed {
                    FixtureObservation::Prepared => {}
                    FixtureObservation::Ready => self.evidence.ready = true,
                    FixtureObservation::Responded => self.evidence.responded()?,
                }
            }
        }
        if !self.closing {
            self.enroll_root()?;
        }
        if !self.closing && self.evidence.can_challenge() {
            self.session
                .as_ref()
                .ok_or("Descendant challenge session")?
                .control
                .request_challenge()?;
            self.evidence.challenge = true;
        }
        self.cleanup()?;
        if self.registry.is_empty()
            && self.key.is_none()
            && self.evidence.retired
            && let Some(session) = &self.session
        {
            session.control.cancel();
        }
        if let Some(session) = &mut self.session
            && let Some(completion) = session.completion.try_take_completion()
        {
            self.session = None;
            match completion.outcome {
                JobOutcome::Cancelled if self.closing || self.evidence.retired => {
                    self.session_retired = true
                }
                JobOutcome::Failure(cause) => return Err(Box::new(cause)),
                _ => {
                    return Err(
                        "Descendant event session ended before intended cancellation".into()
                    );
                }
            }
        }
        if Instant::now() >= self.original && !self.deadline_observed {
            self.deadline_observed = true;
            return Err("Descendant campaign exceeded its original60-second deadline".into());
        }
        Ok(if self.closing || (self.evidence.retired && self.session_retired) {
            HeadlessStep::Shutdown
        } else {
            HeadlessStep::Active
        })
    }
}
