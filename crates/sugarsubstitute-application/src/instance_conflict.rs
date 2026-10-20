// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep force termination an explicit choice, separate from ordinary shutdown and waiting.
use std::time::{Duration, Instant};
use sugarsubstitute_contracts::instance_conflict::{ConflictAction, ConflictOperation as Op};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Current conflict-resolution phase; elapsed time alone never authorizes termination.
pub enum Phase {
    /// Verifying actual ownership.
    Checking,
    /// Verified owner awaiting a user decision.
    Ready,
    /// Ordinary shutdown requested at the retained monotonic instant.
    Closing(Instant),
    /// Explicit force request in progress.
    Forcing,
    /// An original operation failed; cancellation still retires custody.
    Failed,
    /// Retiring custody; the flag records whether continuation was requested.
    Retiring(bool),
    /// All work retired; the flag records continuation.
    Done(bool),
}
impl Phase {
    /// Offer force only after ordinary shutdown has been given fifteen seconds.
    pub fn can_force(self, now: Instant) -> bool {
        matches!(self, Self::Closing(started) if now.saturating_duration_since(started) >= Duration::from_secs(15))
    }
    /// Apply an explicit action and return the native operation it authorizes.
    pub fn choose(&mut self, action: ConflictAction, now: Instant) -> Option<Op> {
        match action {
            ConflictAction::Cancel if *self == Self::Retiring(true) => {
                *self = Self::Retiring(false);
                None
            }
            ConflictAction::Close if *self == Self::Ready => {
                *self = Self::Closing(now);
                Some(Op::Close)
            }
            ConflictAction::Force if self.can_force(now) => {
                *self = Self::Forcing;
                Some(Op::Force)
            }
            ConflictAction::Cancel if !matches!(self, Self::Done(_) | Self::Retiring(_)) => {
                *self = Self::Retiring(false);
                Some(Op::Retire)
            }
            _ => None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn force_requires_verified_owner_normal_close_wait_and_a_separate_choice() {
        let now = Instant::now();
        for mut phase in [
            Phase::Checking,
            Phase::Ready,
            Phase::Failed,
            Phase::Retiring(false),
            Phase::Done(true),
        ] {
            assert_eq!(phase.choose(ConflictAction::Force, now + Duration::from_secs(100)), None);
        }
        let mut phase = Phase::Ready;
        assert_eq!(phase.choose(ConflictAction::Close, now), Some(Op::Close));
        assert_eq!(phase.choose(ConflictAction::Force, now + Duration::from_secs(14)), None);
        assert!(phase.can_force(now + Duration::from_secs(15)));
        assert!(matches!(phase, Phase::Closing(_)));
        assert_eq!(
            phase.choose(ConflictAction::Force, now + Duration::from_secs(15)),
            Some(Op::Force)
        );
        assert_eq!(phase.choose(ConflictAction::Force, now + Duration::from_secs(30)), None);
    }
    #[test]
    fn cancellation_never_implies_termination_or_launch() {
        let now = Instant::now();
        for mut phase in
            [Phase::Checking, Phase::Ready, Phase::Closing(now), Phase::Forcing, Phase::Failed]
        {
            assert_eq!(phase.choose(ConflictAction::Cancel, now), Some(Op::Retire));
            assert_eq!(phase, Phase::Retiring(false));
            assert_eq!(phase.choose(ConflictAction::Close, now), None);
            assert_eq!(phase.choose(ConflictAction::Force, now + Duration::from_secs(99)), None);
        }
        let mut retiring = Phase::Retiring(true);
        assert_eq!(retiring.choose(ConflictAction::Cancel, now), None);
        assert_eq!(retiring, Phase::Retiring(false));
    }
}
