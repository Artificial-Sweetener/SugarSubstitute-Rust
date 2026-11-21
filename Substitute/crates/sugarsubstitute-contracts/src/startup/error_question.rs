// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A failed start that retrying by itself cannot fix, asked over the splash: what failed and what
//! to do about it, with Retry and Close.

use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

/// The person's decision in a startup error dialog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupErrorDecision {
    /// Start again, after fixing what the dialog explained.
    Retry,
    /// Close the application.
    Close,
}

const UNANSWERED: u8 = 0;
const RETRY: u8 = 1;
const CLOSE: u8 = 2;

/// The decision shared between the dialog that records it and the application that acts on it.
/// Both sides live on the event-loop thread; the atomic only makes the handle shareable.
#[derive(Clone, Debug, Default)]
pub struct StartupErrorAnswer(Arc<AtomicU8>);

impl StartupErrorAnswer {
    /// Record `decision`, replacing any decision not yet taken.
    pub fn answer(&self, decision: StartupErrorDecision) {
        self.0.store(
            match decision {
                StartupErrorDecision::Retry => RETRY,
                StartupErrorDecision::Close => CLOSE,
            },
            Ordering::Release,
        );
    }

    /// Take the recorded decision once.
    pub fn take(&self) -> Option<StartupErrorDecision> {
        match self.0.swap(UNANSWERED, Ordering::AcqRel) {
            RETRY => Some(StartupErrorDecision::Retry),
            CLOSE => Some(StartupErrorDecision::Close),
            _ => None,
        }
    }
}

/// What a startup error dialog shows, in the person's language, and where its answer goes.
#[derive(Clone, Debug)]
pub struct StartupErrorQuestion {
    /// Dialog heading.
    pub title: &'static str,
    /// What failed and what the person can do about it.
    pub message: &'static str,
    /// The button that starts again.
    pub retry: &'static str,
    /// The button that closes the application.
    pub close: &'static str,
    /// Where the dialog records the decision.
    pub answer: StartupErrorAnswer,
}

#[cfg(test)]
mod tests {
    use super::{StartupErrorAnswer, StartupErrorDecision};

    /// A decision is taken exactly once, and the last decision before it is taken wins.
    #[test]
    fn a_decision_is_taken_once() {
        let answer = StartupErrorAnswer::default();
        let dialog = answer.clone();
        assert_eq!(answer.take(), None);
        dialog.answer(StartupErrorDecision::Close);
        dialog.answer(StartupErrorDecision::Retry);
        assert_eq!(answer.take(), Some(StartupErrorDecision::Retry));
        assert_eq!(answer.take(), None);
        dialog.answer(StartupErrorDecision::Close);
        assert_eq!(answer.take(), Some(StartupErrorDecision::Close));
    }
}
