// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Submit one finite family cleanup at a time using observed progress and original budgets.
use super::*;
use sugarsubstitute_contracts::lifecycle::ProcessKey;

impl Cleanup {
    /// Rearm only when completed inventory introduces an unbound member watch.
    pub(super) fn inventory_completed(
        &mut self,
        kind: TerminalKind,
        watch_required: bool,
        pending: bool,
        settled: bool,
    ) {
        self.requested |= kind == TerminalKind::Success && watch_required && !pending && !settled;
    }
    /// A cancelled unbound permit is retirement evidence, not a new member exit.
    pub(super) fn member_completed(&mut self, kind: TerminalKind) {
        self.requested |= kind == TerminalKind::Success;
    }
}

impl Fixture {
    /// Read the exact retained fixture identity without a native query.
    pub(super) fn process(&self, index: usize) -> Option<ProcessKey> {
        if index == SENTINEL { self.sentinel.process } else { self.cases[index].evidence.process }
    }
    /// Retain the original10/15-second transaction before validating receipt proof.
    pub(super) fn begin_stop(&mut self, index: usize) -> Result<bool> {
        let Some(key) = self.process(index) else { return Ok(false) };
        if self.cleanup[index].stop.is_some() {
            return Ok(false);
        }
        let plan = StopPlan::new(self.clock.now(), campaign_watch::native_now())?;
        let shutdown = ShutdownId::new(u64::try_from(index)? + 1)?;
        self.registry.as_mut().ok_or("Actual registry is not prepared")?.begin_stop(
            key,
            shutdown,
            plan.budget,
        )?;
        self.cleanup[index].stop = Some(plan);
        self.cleanup[index].requested = true;
        if index == SENTINEL {
            self.sentinel.stop =
                self.cleanup[index].stop.as_ref().map(|stop| (shutdown, stop.budget));
        } else {
            self.cases[index].evidence.begin_stop(shutdown)?;
        }
        Ok(true)
    }
    /// Consume original boundaries once and retire only actually settled records.
    pub(super) fn retire_or_expire(&mut self, index: usize) -> Result<bool> {
        let Some(key) = self.process(index) else { return Ok(false) };
        let retired = if index == SENTINEL {
            self.sentinel.retired
        } else {
            self.cases[index].evidence.retired
        };
        if retired {
            return Ok(false);
        }
        let snapshot =
            self.registry.as_ref().ok_or("Actual registry is not prepared")?.snapshot(key)?;
        if let Some(stop) = &mut self.cleanup[index].stop {
            let due = stop.due(self.clock.now(), snapshot.settled);
            self.cleanup[index].requested |= due.cleanup;
            if due.expired {
                return Err(
                    "Held campaign family exceeded its original15-second settlement budget".into(),
                );
            }
        }
        if snapshot.settled {
            // Transfer every unread fact before retirement; a bounded later turn
            // will drain the remaining classes rather than discard evidence.
            let observed = if index == SENTINEL {
                self.sentinel_settlement_observed
            } else {
                self.cases[index].settlement_observed
            };
            if !observed {
                return Ok(false);
            }
            self.registry.as_mut().ok_or("Actual registry is not prepared")?.retire_settled(key)?;
            if index == SENTINEL {
                self.sentinel.retired = true;
            } else {
                self.cases[index].evidence.retired = true;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
    /// Admit one finite step after new actual evidence or an original budget boundary.
    pub(super) fn cleanup_step(&mut self) -> Result<bool> {
        if self.cleanup_busy.is_some() {
            return Ok(false);
        }
        for _ in 0..=HELD {
            let index = self.cleanup_cursor;
            self.cleanup_cursor = (self.cleanup_cursor + 1) % (HELD + 1);
            if !self.cleanup[index].requested || self.cleanup[index].stop.is_none() {
                continue;
            }
            let Some(key) = self.process(index) else { continue };
            let retired = if index == SENTINEL {
                self.sentinel.retired
            } else {
                self.cases[index].evidence.retired
            };
            if retired {
                continue;
            }
            let snapshot =
                self.registry.as_ref().ok_or("Actual registry is not prepared")?.snapshot(key)?;
            if snapshot.creation_pending || snapshot.cleanup_step_pending || snapshot.settled {
                continue;
            }
            let watch = if snapshot.member_watch_required {
                Some(self.spec(JobClass::NativeObservation, 4096)?)
            } else {
                None
            };
            let spec = self.spec(JobClass::InfrastructureLifecycle, 4096)?;
            self.cleanup[index].requested = false;
            let id = self
                .registry
                .as_mut()
                .ok_or("Actual registry is not prepared")?
                .submit_cleanup_step(
                    self.client.as_ref().ok_or("Campaign client missing")?,
                    key,
                    self.creation_scope.as_ref().ok_or("Campaign cleanup scope missing")?,
                    spec,
                    watch,
                )?;
            self.cleanup[index].member_watch = watch.map(|spec| spec.id);
            self.cleanup_busy = Some((index, id));
            return Ok(true);
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_cleanup_completion_quiesces_until_a_new_native_fact_or_boundary() {
        let mut cleanup = Cleanup { stop: None, requested: false, member_watch: None };
        cleanup.inventory_completed(TerminalKind::Success, false, false, false);
        assert!(!cleanup.requested);
        cleanup.inventory_completed(TerminalKind::Success, true, true, false);
        assert!(!cleanup.requested);
        cleanup.inventory_completed(TerminalKind::Success, true, false, true);
        assert!(!cleanup.requested);
        cleanup.inventory_completed(TerminalKind::Success, true, false, false);
        assert!(cleanup.requested);
    }

    #[test]
    fn an_unchanged_inventory_does_not_erase_an_already_observed_cleanup_need() {
        let mut cleanup = Cleanup { stop: None, requested: true, member_watch: None };
        cleanup.inventory_completed(TerminalKind::Success, false, false, false);
        assert!(cleanup.requested);
    }

    #[test]
    fn failed_duplication_and_unbound_retirement_wait_for_an_original_boundary() {
        let mut cleanup = Cleanup { stop: None, requested: false, member_watch: None };
        cleanup.inventory_completed(TerminalKind::Success, true, false, false);
        assert!(cleanup.requested);
        cleanup.requested = false;
        for kind in [TerminalKind::Failure, TerminalKind::Panicked, TerminalKind::TimedOut] {
            cleanup.inventory_completed(kind, true, false, false);
            cleanup.member_completed(TerminalKind::Cancelled);
            assert!(!cleanup.requested);
        }
        cleanup.member_completed(TerminalKind::Success);
        assert!(cleanup.requested);
    }
}
