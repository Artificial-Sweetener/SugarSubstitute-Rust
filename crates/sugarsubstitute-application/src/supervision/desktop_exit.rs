// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Attribute actual desktop exit and retain independent resource-settlement proof.
use sugarsubstitute_contracts::{execution::Moment, lifecycle::*};

#[derive(Clone, Copy)]
struct ShutdownTransaction {
    id: ShutdownId,
    intent: Option<(CompletionIntent, i64)>,
    budget: ShutdownBudget,
}

/// Observe the transaction without equating expiry or exit to clean settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopExitSnapshot {
    /// Fixed original graceful/settlement deadlines, if shutdown was admitted.
    pub budget: Option<ShutdownBudget>,
    /// Actual native exit attribution; absent until process exit is observed.
    pub exit: Option<DesktopExitKind>,
    /// Actual run-resource settlement has been acknowledged independently.
    pub resources_settled: bool,
    /// Both desktop and supervisor resource proof are complete without cleanup failure.
    pub settled: bool,
    /// Grace elapsed but unsettled ownership remains.
    pub graceful_expired: bool,
    /// Final deadline elapsed but unsettled ownership remains.
    pub deadline_expired: bool,
}
/// Own matching completion proof and actual exit; methods never wait or perform I/O.
///
/// Native adapters retain the process and cleanup registry outside cancellable
/// execution jobs. Authenticated intent/completion arrives through the trusted
/// supervisor endpoint. Raw process exit has no authentication/completion meaning.
pub struct DesktopExitTransaction {
    run: RunKey,
    process: ProcessKey,
    transaction: Option<ShutdownTransaction>,
    completed: bool,
    exit: Option<DesktopExitKind>,
    resources_settled: bool,
    failure: Option<LifecycleFailure>,
    last_now: Option<Moment>,
}
impl DesktopExitTransaction {
    /// Bind a current desktop to its exact native registry entry.
    ///
    /// Rejects foreign owner epochs; the adapter separately verifies desktop/run
    /// binding and ensures process IDs are not reused within that registry epoch.
    pub fn new(run: RunKey, process: ProcessKey) -> Result<Self, LifecycleRejection> {
        if process.owner != run.owner {
            return Err(LifecycleRejection::ForeignOwner);
        }
        Ok(Self {
            run,
            process,
            transaction: None,
            completed: false,
            exit: None,
            resources_settled: false,
            failure: None,
            last_now: None,
        })
    }
    /// Record authenticated current-run intent and freeze a single 10/15-second budget.
    ///
    /// Exact repeats preserve the original budget. Different IDs/intent/status or
    /// intent after actual process exit reject without mutation or deadline reset.
    pub fn begin(
        &mut self,
        run: RunKey,
        shutdown: ShutdownId,
        intent: CompletionIntent,
        expected_code: i64,
        now: Moment,
    ) -> Result<ShutdownBudget, LifecycleRejection> {
        if run != self.run {
            return Err(LifecycleRejection::StaleIdentity);
        }
        self.validate_clock(now)?;
        if let Some(previous) = self.transaction {
            if previous.id != shutdown || previous.intent != Some((intent, expected_code)) {
                return Err(LifecycleRejection::ChangedDuplicate);
            }
            self.last_now = Some(now);
            return Ok(previous.budget);
        }
        if self.exit.is_some() {
            return Err(LifecycleRejection::WrongPhase);
        }
        let budget = ShutdownBudget::new(now)?;
        self.transaction = Some(ShutdownTransaction {
            id: shutdown,
            intent: Some((intent, expected_code)),
            budget,
        });
        self.last_now = Some(now);
        Ok(budget)
    }
    /// Begin supervisor cleanup after unexpected exit without fabricating desktop intent.
    ///
    /// Matching repeats keep the one original absolute budget. A previously started
    /// desktop transaction retains its existing budget. Post-exit cleanup never
    /// changes exit attribution or admits a late normal-completion acknowledgement.
    pub fn begin_cleanup(
        &mut self,
        run: RunKey,
        shutdown: ShutdownId,
        now: Moment,
    ) -> Result<ShutdownBudget, LifecycleRejection> {
        if run != self.run {
            return Err(LifecycleRejection::StaleIdentity);
        }
        self.validate_clock(now)?;
        if self.exit.is_none() {
            return Err(LifecycleRejection::WrongPhase);
        }
        if let Some(previous) = self.transaction {
            if previous.id != shutdown {
                return Err(LifecycleRejection::ChangedDuplicate);
            }
            self.last_now = Some(now);
            return Ok(previous.budget);
        }
        let budget = ShutdownBudget::new(now)?;
        self.transaction = Some(ShutdownTransaction { id: shutdown, intent: None, budget });
        self.last_now = Some(now);
        Ok(budget)
    }
    /// Admit trusted proof while returning rejected native causes intact.
    ///
    /// A hard zero exit is unexpected without preceding matching intent/completion.
    /// Late completion cannot retroactively classify a hard exit as normal.
    /// Cleanup settlement remains a separate observation, not a kill/cancel acknowledgement.
    pub fn observe(
        &mut self,
        observation: DesktopObservation,
    ) -> Result<(), RejectedObservation<DesktopObservation>> {
        let reason = match &observation {
            DesktopObservation::Exited { process, .. } => {
                if *process != self.process {
                    Some(LifecycleRejection::StaleIdentity)
                } else if self.exit.is_some() {
                    Some(LifecycleRejection::WrongPhase)
                } else {
                    None
                }
            }
            DesktopObservation::Completed { run, shutdown }
            | DesktopObservation::ResourcesSettled { run, shutdown }
            | DesktopObservation::Failed { run, shutdown, .. } => {
                if *run != self.run
                    || self.transaction.is_none_or(|transaction| transaction.id != *shutdown)
                {
                    Some(LifecycleRejection::StaleIdentity)
                } else if matches!(observation, DesktopObservation::Completed { .. })
                    && (self.completed || self.exit.is_some())
                    || matches!(observation, DesktopObservation::ResourcesSettled { .. })
                        && self.resources_settled
                    || matches!(observation, DesktopObservation::Failed { .. })
                        && self.failure.is_some()
                {
                    Some(LifecycleRejection::WrongPhase)
                } else {
                    None
                }
            }
        };
        if let Some(reason) = reason {
            return Err(RejectedObservation { reason, observation });
        }
        match observation {
            DesktopObservation::Completed { .. } => self.completed = true,
            DesktopObservation::Exited { code, .. } => {
                self.exit = Some(if self.completed {
                    match self.transaction.and_then(|transaction| transaction.intent) {
                        Some((intent, expected)) if code == expected => match intent {
                            CompletionIntent::Normal => DesktopExitKind::Normal,
                            CompletionIntent::StartupCancelled => DesktopExitKind::StartupCancelled,
                            CompletionIntent::StartupFailed => DesktopExitKind::StartupFailed,
                        },
                        _ => DesktopExitKind::Unexpected,
                    }
                } else {
                    DesktopExitKind::Unexpected
                });
            }
            DesktopObservation::ResourcesSettled { .. } => self.resources_settled = true,
            DesktopObservation::Failed { failure, .. } => self.failure = Some(failure),
        }
        Ok(())
    }
    /// Borrow the original first cleanup failure; formatting is redacted by its owner.
    pub fn failure(&self) -> Option<&LifecycleFailure> {
        self.failure.as_ref()
    }
    /// Inspect fixed deadlines using the same injected monotonic epoch.
    ///
    /// Regressing time rejects without mutation. Expiry never removes native ownership
    /// or substitutes for settlement. No implicit deadline restart or timer is created.
    pub fn poll(&mut self, now: Moment) -> Result<DesktopExitSnapshot, LifecycleRejection> {
        self.validate_clock(now)?;
        self.last_now = Some(now);
        let budget = self.transaction.map(|transaction| transaction.budget);
        let settled = self.exit.is_some() && self.resources_settled && self.failure.is_none();
        Ok(DesktopExitSnapshot {
            budget,
            exit: self.exit,
            resources_settled: self.resources_settled,
            settled,
            graceful_expired: !settled && budget.is_some_and(|b| now >= b.graceful_until),
            deadline_expired: !settled && budget.is_some_and(|b| now >= b.settle_until),
        })
    }
    fn validate_clock(&self, now: Moment) -> Result<(), LifecycleRejection> {
        if self.last_now.is_some_and(|previous| now < previous) {
            Err(LifecycleRejection::InvalidClock)
        } else {
            Ok(())
        }
    }
}
