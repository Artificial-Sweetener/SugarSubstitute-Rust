// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve original stop budgets and actual root-versus-family facts through cleanup.
use super::{Result, evidence::Stop, options::Role, parent::Parent};
use std::time::Instant;
use sugarsubstitute_contracts::{
    execution::{Clock, JobClass},
    lifecycle::ShutdownId,
};
use sugarsubstitute_supervision::RegistryObservation;
impl Parent {
    pub(super) fn ensure_stop(&mut self) -> Result<()> {
        let Some(case) = self.cases.get_mut(self.current) else { return Ok(()) };
        if case.retired {
            return Ok(());
        }
        let Some(key) = case.process else { return Ok(()) };
        if case.needs_stop(self.registry.snapshot(key)?) {
            let stop = Stop::new(
                ShutdownId::new(self.current as u64 + 1)?,
                self.clock.now(),
                Instant::now(),
            )?;
            self.registry.begin_stop(key, stop.id, stop.budget)?;
            case.stop = Some(stop);
            case.cleanup_requested = true;
        }
        Ok(())
    }
    pub(super) fn progress_family(&mut self) -> Result<()> {
        let Some(key) =
            self.cases[self.current].process.filter(|_| !self.cases[self.current].retired)
        else {
            return Ok(());
        };
        for _ in 0..5 {
            let Some(observation) = self.registry.take_observation(key)? else { break };
            let case = &mut self.cases[self.current];
            match observation {
                RegistryObservation::ContainedStarted { process }
                    if process == key && !case.started =>
                {
                    case.started = true
                }
                RegistryObservation::RootExited { process, code }
                    if process == key && case.exited.is_none() =>
                {
                    case.exited = Some(code);
                    case.cleanup_requested = true;
                    if case.role == Role::Forward {
                        let report = case
                            .output
                            .as_ref()
                            .ok_or("Descendant output missing")?
                            .control
                            .snapshot();
                        case.root_before_eof =
                            report.eof == [false, false] && case.output_terminal.is_none();
                        if !case.root_before_eof {
                            return Err("Root exit did not precede descendant writer EOF".into());
                        }
                    }
                    if code != 0 {
                        return Err("Output self child root exit was not zero".into());
                    }
                }
                RegistryObservation::FamilySettled { process, shutdown } if process == key => {
                    case.settled = true;
                    if shutdown != case.stop.as_ref().map(|stop| stop.id) {
                        return Err("Output family settlement transaction differs".into());
                    }
                }
                RegistryObservation::Failed { error, .. } => return Err(Box::new(error)),
                _ => return Err("Output family observation identity/order differs".into()),
            }
        }
        if self.closing || self.cases[self.current].exited.is_some() {
            self.ensure_stop()?;
        }
        let snapshot = self.registry.snapshot(key)?;
        let case = &mut self.cases[self.current];
        case.reconsider_cleanup(snapshot);
        if snapshot.forced {
            case.forced = true;
        }
        if case.role == Role::Forward && case.root_before_eof {
            if snapshot.retained_members == 2 && !snapshot.settled {
                case.complete_inventory = true;
            }
            if let Some(exit) = snapshot.last_member_exit {
                let expected = case
                    .channel
                    .as_ref()
                    .and_then(|channel| channel.descendant)
                    .ok_or("Actual declared descendant identity missing")?;
                if (exit.process_id, exit.creation_time_100ns) != expected
                    || exit.exit_code != 1
                    || !case.forced
                {
                    return Err(
                        "Actual descendant exit differs from exact retained identity/owned stop"
                            .into(),
                    );
                }
                case.member_verified = true;
            }
        }
        if let Some(stop) = &mut case.stop {
            let now = self.clock.now();
            let (grace, total) = stop.due(now);
            if grace && !snapshot.settled {
                case.cleanup_requested = true;
            }
            if total && !snapshot.settled {
                return Err(
                    "Output family remains owned after original15s settlement budget".into()
                );
            }
        }
        if snapshot.settled {
            self.registry.retire_settled(key)?;
            case.retired = true;
            if !self.closing
                && (!case.settled
                    || case.role == Role::Forward
                        && !(case.complete_inventory && case.member_verified && case.forced))
            {
                return Err("Output family retired without required actual proof".into());
            }
        } else if case.can_submit_cleanup(snapshot) {
            let member = snapshot.member_watch_required;
            let watch = if member {
                Some(self.spec(JobClass::NativeObservation, 4096, false)?)
            } else {
                None
            };
            let step = self.spec(JobClass::InfrastructureLifecycle, 4096, false)?;
            let id = self.registry.submit_cleanup_step(
                self.client.as_ref().ok_or("Output cleanup client missing")?,
                key,
                self.scope.as_ref().ok_or("Output cleanup scope missing")?,
                step,
                watch,
            )?;
            let case = &mut self.cases[self.current];
            case.record_cleanup_admission(id, watch.map(|spec| spec.id));
        }
        Ok(())
    }
}
impl super::evidence::Case {
    /// A finite step preserves an already admitted member watch until its exact notice.
    pub(super) fn record_cleanup_admission(
        &mut self,
        cleanup: sugarsubstitute_contracts::execution::JobId,
        member: Option<sugarsubstitute_contracts::execution::JobId>,
    ) {
        self.cleanup_job = Some(cleanup);
        if let Some(member) = member {
            self.member_job = Some(member);
        }
        self.cleanup_requested = false;
    }
    /// A retained member watch cannot block the finite step at the original grace boundary.
    pub(super) fn can_submit_cleanup(
        &self,
        snapshot: sugarsubstitute_supervision::RegistrySnapshot,
    ) -> bool {
        self.stop.is_some()
            && self.cleanup_requested
            && self.cleanup_job.is_none()
            && !snapshot.creation_pending
            && !snapshot.settled
            && !self.retired
    }
    pub(super) fn needs_stop(
        &self,
        snapshot: sugarsubstitute_supervision::RegistrySnapshot,
    ) -> bool {
        !snapshot.settled && self.stop.is_none() && !self.retired
    }
    pub(super) fn reconsider_cleanup(
        &mut self,
        snapshot: sugarsubstitute_supervision::RegistrySnapshot,
    ) {
        let cleanup = std::mem::take(&mut self.cleanup_success);
        let member = std::mem::take(&mut self.member_success);
        if !snapshot.settled
            && !snapshot.cleanup_pending
            && ((cleanup && snapshot.member_watch_required)
                || (member && snapshot.last_member_exit.is_some()))
        {
            self.cleanup_requested = true;
        }
    }
}
