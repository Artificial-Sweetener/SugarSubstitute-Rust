// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Track script and binding installation through actual callback and checked removal.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Action {
    AddBinding,
    AddScript,
    RemoveScript,
    RemoveBinding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    New,
    AddingBinding,
    Bound,
    AddingScript,
    Ready,
    RemovingScript,
    RemovingBinding,
    Retired,
    Uncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Next {
    Effect(Action),
    Await,
    Ready,
    Retired,
    Uncertain,
}

/// Preserve possibly effective registrations until their exact removal succeeds.
pub(super) struct Ledger {
    phase: Phase,
    closing: bool,
}

impl Ledger {
    pub(super) const fn new() -> Self {
        Self { phase: Phase::New, closing: false }
    }

    /// Fence new setup while retaining any in-flight callback obligation.
    pub(super) fn close(&mut self) {
        self.closing = true;
    }

    /// Classify the next bounded effect without manufacturing a native completion.
    pub(super) fn next(&self) -> Next {
        match (self.phase, self.closing) {
            (Phase::New, false) => Next::Effect(Action::AddBinding),
            (Phase::New | Phase::Retired, true) | (Phase::Retired, false) => Next::Retired,
            (Phase::Bound, false) => Next::Effect(Action::AddScript),
            (Phase::Bound, true) => Next::Effect(Action::RemoveBinding),
            (Phase::Ready, false) => Next::Ready,
            (Phase::Ready, true) => Next::Effect(Action::RemoveScript),
            (Phase::Uncertain, _) => Next::Uncertain,
            (
                Phase::AddingBinding
                | Phase::AddingScript
                | Phase::RemovingScript
                | Phase::RemovingBinding,
                _,
            ) => Next::Await,
        }
    }

    /// Arm the exact obligation before invoking a native method that can complete synchronously.
    pub(super) fn begin(&mut self, action: Action) -> bool {
        if self.next() != Next::Effect(action) {
            return false;
        }
        self.phase = match action {
            Action::AddBinding => Phase::AddingBinding,
            Action::AddScript => Phase::AddingScript,
            Action::RemoveScript => Phase::RemovingScript,
            Action::RemoveBinding => Phase::RemovingBinding,
        };
        true
    }

    /// Accept only the matching actual callback, retaining uncertain or failed effects.
    pub(super) fn complete(&mut self, action: Action, success: bool) -> bool {
        let expected = match action {
            Action::AddBinding => Phase::AddingBinding,
            Action::AddScript => Phase::AddingScript,
            Action::RemoveScript => Phase::RemovingScript,
            Action::RemoveBinding => Phase::RemovingBinding,
        };
        if self.phase != expected {
            return false;
        }
        self.phase = match (action, success) {
            (Action::AddBinding, _)
            | (Action::RemoveScript, true)
            | (Action::RemoveBinding, false) => Phase::Bound,
            (Action::AddScript, true) | (Action::RemoveScript, false) => Phase::Ready,
            (Action::AddScript, false) => Phase::Uncertain,
            (Action::RemoveBinding, true) => Phase::Retired,
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_script_registration_is_removed_before_its_binding() {
        let mut ledger = Ledger::new();
        assert!(ledger.begin(Action::AddBinding));
        assert!(ledger.complete(Action::AddBinding, true));
        assert!(ledger.begin(Action::AddScript));
        ledger.close();
        assert_eq!(ledger.next(), Next::Await);
        assert!(ledger.complete(Action::AddScript, true));
        assert_eq!(ledger.next(), Next::Effect(Action::RemoveScript));
        assert!(ledger.begin(Action::RemoveScript));
        assert!(!ledger.complete(Action::RemoveBinding, true));
        assert!(ledger.complete(Action::RemoveScript, true));
        assert_eq!(ledger.next(), Next::Effect(Action::RemoveBinding));
        assert!(ledger.begin(Action::RemoveBinding));
        assert!(ledger.complete(Action::RemoveBinding, true));
        assert_eq!(ledger.next(), Next::Retired);
    }

    #[test]
    fn failed_removal_retains_the_exact_effect_and_unknown_script_registration_never_settles() {
        let mut ledger = Ledger::new();
        assert!(ledger.begin(Action::AddBinding));
        assert!(ledger.complete(Action::AddBinding, true));
        assert!(ledger.begin(Action::AddScript));
        assert!(ledger.complete(Action::AddScript, true));
        ledger.close();
        assert!(ledger.begin(Action::RemoveScript));
        assert!(ledger.complete(Action::RemoveScript, false));
        assert_eq!(ledger.next(), Next::Effect(Action::RemoveScript));

        let mut uncertain = Ledger::new();
        assert!(uncertain.begin(Action::AddBinding));
        assert!(uncertain.complete(Action::AddBinding, true));
        assert!(uncertain.begin(Action::AddScript));
        assert!(uncertain.complete(Action::AddScript, false));
        uncertain.close();
        assert_eq!(uncertain.next(), Next::Uncertain);
    }
}
