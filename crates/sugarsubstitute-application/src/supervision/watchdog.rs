// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own bounded per-run native UI challenge policy without timers or automatic recovery.
use sugarsubstitute_contracts::{execution::Moment, lifecycle::*};

/// Transmission phase of the one retained challenge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiChallengePhase {
    /// Effect emitted; actual native transmission is still unproved.
    Queued,
    /// Actual native dispatch has been acknowledged independently.
    Dispatched,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn final_challenge_identity_is_issued_once_then_exhaustion_preserves_state()
    -> Result<(), Box<dyn Error>> {
        let run = RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        };
        let zero = Moment::from_nanos(0);
        let five = Moment::from_nanos(5_000_000_000);
        let ten = Moment::from_nanos(10_000_000_000);
        let mut watchdog = UiWatchdog::new(run, zero);
        watchdog.last_identity = u64::MAX - 1;
        watchdog.set_activity(run, UiWatchdogActivity::VisibleActive, zero)?;
        let Some(UiWatchdogEffect::Challenge { challenge }) = watchdog.poll(five)? else {
            return Err("expected last representable challenge".into());
        };
        assert_eq!(challenge.id.get(), u64::MAX);
        watchdog.observe(run, UiWatchdogObservation::Dispatched { challenge, at: five }, five)?;
        watchdog.observe(run, UiWatchdogObservation::Responded { challenge }, five)?;
        let before = watchdog.snapshot();
        assert_eq!(watchdog.poll(ten), Err(LifecycleRejection::IdentityExhausted));
        assert_eq!(watchdog.snapshot(), before);
        assert_eq!(watchdog.last_now, five);
        assert_eq!(watchdog.last_identity, u64::MAX);
        Ok(())
    }
}
#[derive(Clone, Copy)]
struct PendingChallenge {
    key: UiChallengeKey,
    queued_at: Moment,
    suspicion_at: Option<Moment>,
    diagnostic_emitted: bool,
}
/// Inspect bounded policy state and schedule polls in the same injected clock epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiWatchdogSnapshot {
    /// Native eligibility, distinct from web/backend activity.
    pub activity: UiWatchdogActivity,
    /// Exact current challenge, absent after invalidation or a matching response.
    pub pending: Option<UiChallengeKey>,
    /// Current transmission phase, absent when no challenge is pending.
    pub phase: Option<UiChallengePhase>,
    /// Time the one challenge effect was issued, not actual transmission time.
    pub queued_at: Option<Moment>,
    /// Absolute actual-dispatch-plus-thirty-second suspicion deadline, if dispatched.
    pub suspicion_at: Option<Moment>,
    /// Absolute five-second cadence boundary when active without a pending challenge.
    pub next_challenge_at: Option<Moment>,
    /// The one diagnostic effect has been issued; it is not proof of a hung process.
    pub suspicious: bool,
}
/// Monitor one exact desktop run through actual native UI-loop challenge evidence.
///
/// All methods are immediate policy transitions; no I/O, scheduling or native handles
/// are owned here. One pending record is retained, including after suspicion. Native
/// adapters authenticate the run separately and serialize `validate_dispatch` plus
/// actual transmission against activity changes. Dropped effects are not dispatch.
pub struct UiWatchdog {
    run: RunKey,
    activity: UiWatchdogActivity,
    pending: Option<PendingChallenge>,
    next_challenge_at: Option<Moment>,
    last_identity: u64,
    last_now: Moment,
}
impl UiWatchdog {
    /// Start inactive using the composing owner's injected monotonic epoch.
    pub const fn new(run: RunKey, now: Moment) -> Self {
        Self {
            run,
            activity: UiWatchdogActivity::Inactive,
            pending: None,
            next_challenge_at: None,
            last_identity: 0,
            last_now: now,
        }
    }
    /// Apply trusted native visibility/inactivity or acknowledged sleep/modal changes.
    ///
    /// A changed activity invalidates queued and dispatched keys. Resume begins a
    /// fresh five-second cadence. Identical activity never resets a pending challenge
    /// or deadline. Stale runs, regression and deadline overflow reject atomically.
    pub fn set_activity(
        &mut self,
        trusted_run: RunKey,
        activity: UiWatchdogActivity,
        now: Moment,
    ) -> Result<(), LifecycleRejection> {
        self.validate_run(trusted_run)?;
        self.validate_clock(now)?;
        if activity != self.activity {
            let next = if activity == UiWatchdogActivity::VisibleActive {
                Some(
                    now.checked_add(UI_CHALLENGE_CADENCE)
                        .map_err(|_| LifecycleRejection::InvalidClock)?,
                )
            } else {
                None
            };
            self.activity = activity;
            self.pending = None;
            self.next_challenge_at = next;
        }
        self.last_now = now;
        Ok(())
    }
    /// Revalidate one queued effect immediately before actual native transmission.
    ///
    /// The adapter MUST serialize this check and transmit against activity changes.
    /// Success proves only current policy eligibility, never privilege/authentication
    /// or successful transmission. This read-only check neither samples nor advances time.
    pub fn validate_dispatch(
        &self,
        trusted_run: RunKey,
        challenge: UiChallengeKey,
    ) -> Result<(), LifecycleRejection> {
        self.validate_run(trusted_run)?;
        self.validate_key(challenge)?;
        if self.activity != UiWatchdogActivity::VisibleActive
            || self.pending.is_none_or(|pending| pending.suspicion_at.is_some())
        {
            return Err(LifecycleRejection::WrongPhase);
        }
        Ok(())
    }
    /// Admit separately authenticated actual dispatch or actual native UI-loop response.
    ///
    /// Dispatch time must lie between effect issuance and receipt `now`, in the same
    /// clock epoch; delayed reports retain the original thirty-second deadline.
    /// Response requires dispatched phase. The adapter retains early replies until
    /// their dispatch fact has been delivered. Stale/foreign keys and invalid clocks
    /// reject without mutation. A late matching response clears suspicion normally.
    pub fn observe(
        &mut self,
        trusted_run: RunKey,
        observation: UiWatchdogObservation,
        now: Moment,
    ) -> Result<(), LifecycleRejection> {
        self.validate_run(trusted_run)?;
        self.validate_clock(now)?;
        let challenge = match observation {
            UiWatchdogObservation::Dispatched { challenge, .. }
            | UiWatchdogObservation::Responded { challenge } => challenge,
        };
        self.validate_key(challenge)?;
        let Some(pending) = self.pending else {
            return Err(LifecycleRejection::StaleIdentity);
        };
        match observation {
            UiWatchdogObservation::Dispatched { at, .. } => {
                if pending.suspicion_at.is_some() {
                    return Err(LifecycleRejection::WrongPhase);
                }
                if at < pending.queued_at || at > now {
                    return Err(LifecycleRejection::InvalidClock);
                }
                let deadline = at
                    .checked_add(UI_SUSPICION_THRESHOLD)
                    .map_err(|_| LifecycleRejection::InvalidClock)?;
                self.pending = Some(PendingChallenge { suspicion_at: Some(deadline), ..pending });
            }
            UiWatchdogObservation::Responded { .. } => {
                if pending.suspicion_at.is_none() {
                    return Err(LifecycleRejection::WrongPhase);
                }
                let next = now
                    .checked_add(UI_CHALLENGE_CADENCE)
                    .map_err(|_| LifecycleRejection::InvalidClock)?;
                self.pending = None;
                self.next_challenge_at = Some(next);
            }
        }
        self.last_now = now;
        Ok(())
    }
    /// Emit one challenge at cadence, or one diagnostic at actual-dispatch deadline.
    ///
    /// Pending queued effects never cause suspicion or additional challenges. Expiry
    /// retains ownership and never stops/restarts a process or replays inference.
    /// Regression and identity exhaustion reject without changing policy state.
    pub fn poll(&mut self, now: Moment) -> Result<Option<UiWatchdogEffect>, LifecycleRejection> {
        self.validate_clock(now)?;
        let effect = if self.activity != UiWatchdogActivity::VisibleActive {
            None
        } else if let Some(pending) = self.pending.as_mut() {
            if !pending.diagnostic_emitted && pending.suspicion_at.is_some_and(|at| now >= at) {
                pending.diagnostic_emitted = true;
                Some(UiWatchdogEffect::Unresponsive { challenge: pending.key })
            } else {
                None
            }
        } else if self.next_challenge_at.is_some_and(|at| now >= at) {
            let id =
                self.last_identity.checked_add(1).ok_or(LifecycleRejection::IdentityExhausted)?;
            let challenge = UiChallengeKey {
                run: self.run,
                id: UiChallengeId::new(id).map_err(|_| LifecycleRejection::IdentityExhausted)?,
            };
            self.pending = Some(PendingChallenge {
                key: challenge,
                queued_at: now,
                suspicion_at: None,
                diagnostic_emitted: false,
            });
            self.next_challenge_at = None;
            self.last_identity = id;
            Some(UiWatchdogEffect::Challenge { challenge })
        } else {
            None
        };
        self.last_now = now;
        Ok(effect)
    }
    /// Borrow-free facts for scheduling and serialized queued-effect validation.
    pub fn snapshot(&self) -> UiWatchdogSnapshot {
        UiWatchdogSnapshot {
            activity: self.activity,
            pending: self.pending.map(|p| p.key),
            phase: self.pending.map(|p| {
                if p.suspicion_at.is_some() {
                    UiChallengePhase::Dispatched
                } else {
                    UiChallengePhase::Queued
                }
            }),
            queued_at: self.pending.map(|p| p.queued_at),
            suspicion_at: self.pending.and_then(|p| p.suspicion_at),
            next_challenge_at: self.next_challenge_at,
            suspicious: self.pending.is_some_and(|p| p.diagnostic_emitted),
        }
    }
    fn validate_run(&self, run: RunKey) -> Result<(), LifecycleRejection> {
        if run.owner != self.run.owner {
            Err(LifecycleRejection::ForeignOwner)
        } else if run != self.run {
            Err(LifecycleRejection::StaleIdentity)
        } else {
            Ok(())
        }
    }
    fn validate_key(&self, key: UiChallengeKey) -> Result<(), LifecycleRejection> {
        self.validate_run(key.run)?;
        if self.pending.is_some_and(|p| p.key == key) {
            Ok(())
        } else {
            Err(LifecycleRejection::StaleIdentity)
        }
    }
    fn validate_clock(&self, now: Moment) -> Result<(), LifecycleRejection> {
        if now < self.last_now { Err(LifecycleRejection::InvalidClock) } else { Ok(()) }
    }
}
