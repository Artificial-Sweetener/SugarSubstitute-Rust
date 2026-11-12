// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep force termination an explicit choice, separate from ordinary shutdown and waiting.
//!
//! A previous copy that is closing by itself is waited for without a question. A previous
//! copy that is not responding is ended only after the user chooses to close it and start.
use std::time::Duration;
use sugarsubstitute_contracts::execution::Moment;
use sugarsubstitute_contracts::instance_conflict::{
    ConflictAction, ConflictOperation as Op, ConflictPhase, ConflictView,
};

/// Ordinary shutdown is given this long before ending the previous copy outright is offered.
const FORCE_OFFER_DELAY: Duration = Duration::from_secs(15);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Current conflict-resolution phase; elapsed time alone never authorizes termination.
pub enum Phase {
    /// Verifying actual ownership.
    Checking,
    /// Verified owner awaiting a user decision.
    Ready,
    /// Verified owner is not responding; closing it ends it outright, after the user chooses.
    Unresponsive,
    /// Verified owner is closing by itself; only cancellation is offered while it exits.
    AwaitingExit,
    /// Ordinary shutdown requested at the retained moment of the controller's execution clock.
    Closing(Moment),
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
    ///
    /// `now` comes from the same execution clock that stamped [`Phase::Closing`]; a clock that
    /// reads earlier than that stamp never offers force.
    pub fn can_force(self, now: Moment) -> bool {
        matches!(self, Self::Closing(started)
            if now.checked_duration_since(started).is_ok_and(|waited| waited >= FORCE_OFFER_DELAY))
    }
    /// Apply an explicit action and return the native operation it authorizes.
    pub fn choose(&mut self, action: ConflictAction, now: Moment) -> Option<Op> {
        match action {
            ConflictAction::Cancel if *self == Self::Retiring(true) => {
                *self = Self::Retiring(false);
                None
            }
            ConflictAction::Close if *self == Self::Ready => {
                *self = Self::Closing(now);
                Some(Op::Close)
            }
            ConflictAction::Close if *self == Self::Unresponsive => {
                *self = Self::Forcing;
                Some(Op::Force)
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
    /// Project the phase a conflict surface presents at `now`, without native authority.
    pub fn view(self, now: Moment) -> ConflictView {
        let phase = match self {
            Self::Checking => ConflictPhase::Checking,
            Self::Ready | Self::Unresponsive => ConflictPhase::AwaitingDecision,
            Self::AwaitingExit => ConflictPhase::AwaitingExit,
            Self::Closing(_) if self.can_force(now) => ConflictPhase::ForceOffered,
            Self::Closing(_) => ConflictPhase::Closing,
            Self::Forcing => ConflictPhase::Forcing,
            Self::Failed => ConflictPhase::Failed,
            Self::Retiring(continuing) => ConflictPhase::Retiring { continuing },
            Self::Done(_) => ConflictPhase::Done,
        };
        ConflictView { phase }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn at(seconds: u64) -> Moment {
        Moment::from_nanos(seconds * 1_000_000_000)
    }
    #[test]
    fn force_requires_verified_owner_normal_close_wait_and_a_separate_choice() {
        for mut phase in [
            Phase::Checking,
            Phase::Ready,
            Phase::Failed,
            Phase::Retiring(false),
            Phase::Done(true),
        ] {
            assert_eq!(phase.choose(ConflictAction::Force, at(100)), None);
        }
        let mut phase = Phase::Ready;
        assert_eq!(phase.choose(ConflictAction::Close, at(1)), Some(Op::Close));
        assert_eq!(phase.choose(ConflictAction::Force, at(15)), None);
        assert!(phase.can_force(at(16)));
        assert!(matches!(phase, Phase::Closing(_)));
        assert_eq!(phase.choose(ConflictAction::Force, at(16)), Some(Op::Force));
        assert_eq!(phase.choose(ConflictAction::Force, at(30)), None);
    }
    /// An unresponsive copy is ended only by the user's choice, at once; a closing copy offers
    /// nothing but Cancel and never ends anything.
    #[test]
    fn unresponsive_copy_ends_on_choice_and_closing_copy_is_only_waited_for() {
        let mut unresponsive = Phase::Unresponsive;
        assert_eq!(unresponsive.view(at(0)).phase, ConflictPhase::AwaitingDecision);
        assert!(unresponsive.view(at(0)).close_enabled());
        assert_eq!(unresponsive.choose(ConflictAction::Force, at(0)), None);
        assert_eq!(unresponsive.choose(ConflictAction::Close, at(0)), Some(Op::Force));
        assert_eq!(unresponsive, Phase::Forcing);
        let mut closing = Phase::AwaitingExit;
        let view = closing.view(at(100));
        assert_eq!(view.phase, ConflictPhase::AwaitingExit);
        assert!(!view.close_enabled() && !view.force_enabled() && !view.done());
        assert_eq!(closing.choose(ConflictAction::Close, at(100)), None);
        assert_eq!(closing.choose(ConflictAction::Force, at(100)), None);
        assert_eq!(closing.choose(ConflictAction::Cancel, at(100)), Some(Op::Retire));
        assert_eq!(closing, Phase::Retiring(false));
    }
    #[test]
    fn a_clock_reading_before_the_close_request_never_offers_force() {
        let phase = Phase::Closing(at(20));
        assert!(!phase.can_force(at(5)));
        assert_eq!(phase.view(at(5)).phase, ConflictPhase::Closing);
    }
    #[test]
    fn cancellation_never_implies_termination_or_launch() {
        let now = at(1);
        for mut phase in
            [Phase::Checking, Phase::Ready, Phase::Closing(now), Phase::Forcing, Phase::Failed]
        {
            assert_eq!(phase.choose(ConflictAction::Cancel, now), Some(Op::Retire));
            assert_eq!(phase, Phase::Retiring(false));
            assert_eq!(phase.choose(ConflictAction::Close, now), None);
            assert_eq!(phase.choose(ConflictAction::Force, at(99)), None);
        }
        let mut retiring = Phase::Retiring(true);
        assert_eq!(retiring.choose(ConflictAction::Cancel, now), None);
        assert_eq!(retiring, Phase::Retiring(false));
    }
    #[test]
    fn the_view_names_every_phase_and_offers_only_what_the_phase_permits() {
        let closing = Phase::Closing(at(0));
        let cases = [
            (Phase::Checking, at(0), ConflictPhase::Checking, (false, false, false)),
            (Phase::Ready, at(0), ConflictPhase::AwaitingDecision, (true, false, false)),
            (closing, at(14), ConflictPhase::Closing, (false, false, false)),
            (closing, at(15), ConflictPhase::ForceOffered, (false, true, false)),
            (Phase::Forcing, at(0), ConflictPhase::Forcing, (false, false, false)),
            (Phase::Failed, at(0), ConflictPhase::Failed, (false, false, false)),
            (
                Phase::Retiring(true),
                at(0),
                ConflictPhase::Retiring { continuing: true },
                (false, false, false),
            ),
            (
                Phase::Retiring(false),
                at(0),
                ConflictPhase::Retiring { continuing: false },
                (false, false, false),
            ),
            (Phase::Done(true), at(0), ConflictPhase::Done, (false, false, true)),
        ];
        for (phase, now, expected, (close, force, done)) in cases {
            let view = phase.view(now);
            assert_eq!(view.phase, expected);
            assert_eq!(view.close_enabled(), close, "{expected:?}");
            assert_eq!(view.force_enabled(), force, "{expected:?}");
            assert_eq!(view.done(), done, "{expected:?}");
        }
    }
}
