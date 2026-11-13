// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Track why a native surface must be configured again after its current image retires.

use std::sync::Mutex;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReconfigureReason {
    Initial,
    SizeChanged,
    Suboptimal,
    Outdated,
    Lost,
    Discarded,
}

impl ReconfigureReason {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::SizeChanged => "size_changed",
            Self::Suboptimal => "suboptimal",
            Self::Outdated => "outdated",
            Self::Lost => "lost",
            Self::Discarded => "discarded",
        }
    }

    const fn priority(self) -> u8 {
        match self {
            Self::Initial => 1,
            Self::Discarded => 2,
            Self::Suboptimal => 3,
            Self::Outdated => 4,
            Self::Lost => 5,
            Self::SizeChanged => 6,
        }
    }
}

#[derive(Default)]
pub(crate) struct Reconfiguration(Mutex<Option<ReconfigureReason>>);

impl Reconfiguration {
    pub(crate) fn mark(&self, reason: ReconfigureReason) {
        let mut current = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if current.is_none_or(|current| reason.priority() >= current.priority()) {
            *current = Some(reason);
        }
    }

    pub(crate) fn reason(&self) -> Option<ReconfigureReason> {
        *self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn configured(&self) {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
    }

    pub(crate) fn presented(&self) {
        let mut current = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if *current == Some(ReconfigureReason::Discarded) {
            current.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_presentation_clears_only_the_discard_sentinel() {
        let state = Reconfiguration::default();
        state.mark(ReconfigureReason::Discarded);
        state.presented();
        assert_eq!(state.reason(), None);

        state.mark(ReconfigureReason::Discarded);
        state.mark(ReconfigureReason::Suboptimal);
        state.presented();
        assert_eq!(state.reason(), Some(ReconfigureReason::Suboptimal));
        state.configured();
        assert_eq!(state.reason(), None);
    }
}
