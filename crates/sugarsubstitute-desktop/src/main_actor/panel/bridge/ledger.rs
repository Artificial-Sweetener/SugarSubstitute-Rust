// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Track bounded script and binding custody through actual native completions.

/// Identify one exact native effect, including its bootstrap script slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Action {
    AddBinding,
    AddScript(usize),
    RemoveScript(usize),
    RemoveBinding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    New,
    Bound,
    Pending(Action),
    Retired,
}

/// Classify the next registration effect or retained completion obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Next {
    Effect(Action),
    Await,
    Ready,
    Retired,
    Uncertain,
}

/// Keep known registrations removable even when another registration is uncertain.
pub(super) struct Ledger {
    phase: Phase,
    closing: bool,
    registered: usize,
    script_count: usize,
    uncertain: bool,
}

impl Ledger {
    /// Allocate a fixed bootstrap inventory before any native registration.
    pub(super) const fn new(script_count: usize) -> Self {
        Self { phase: Phase::New, closing: false, registered: 0, script_count, uncertain: false }
    }

    /// Fence new setup while retaining any in-flight callback obligation.
    pub(super) fn close(&mut self) {
        self.closing = true;
    }

    /// Remove known scripts before considering unknown effects or their binding.
    pub(super) fn next(&self) -> Next {
        match self.phase {
            Phase::New if !self.closing => Next::Effect(Action::AddBinding),
            Phase::New | Phase::Retired => Next::Retired,
            Phase::Pending(_) => Next::Await,
            Phase::Bound if self.closing && self.registered > 0 => {
                Next::Effect(Action::RemoveScript(self.registered - 1))
            }
            Phase::Bound if self.uncertain => Next::Uncertain,
            Phase::Bound if self.closing => Next::Effect(Action::RemoveBinding),
            Phase::Bound if self.registered < self.script_count => {
                Next::Effect(Action::AddScript(self.registered))
            }
            Phase::Bound => Next::Ready,
        }
    }

    /// Arm the exact obligation before a native method can complete synchronously.
    pub(super) fn begin(&mut self, action: Action) -> bool {
        if self.next() != Next::Effect(action) {
            return false;
        }
        self.phase = Phase::Pending(action);
        true
    }

    /// Accept only an actual matching callback; failed removals retain their slot.
    pub(super) fn complete(&mut self, action: Action, success: bool) -> bool {
        if self.phase != Phase::Pending(action) {
            return false;
        }
        self.phase = Phase::Bound;
        match (action, success) {
            (Action::AddScript(_), true) => self.registered += 1,
            (Action::AddScript(_), false) => {
                self.uncertain = true;
                self.closing = true;
            }
            (Action::RemoveScript(_), true) => self.registered -= 1,
            (Action::RemoveBinding, true) => self.phase = Phase::Retired,
            _ => {}
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finish(ledger: &mut Ledger, action: Action) {
        assert_eq!(ledger.next(), Next::Effect(action));
        assert!(ledger.begin(action));
        assert_eq!(ledger.next(), Next::Await);
        assert!(ledger.complete(action, true));
    }

    #[test]
    fn scripts_register_in_order_and_retire_in_reverse_before_the_binding() {
        let mut ledger = Ledger::new(2);
        finish(&mut ledger, Action::AddBinding);
        finish(&mut ledger, Action::AddScript(0));
        finish(&mut ledger, Action::AddScript(1));
        assert_eq!(ledger.next(), Next::Ready);
        ledger.close();
        finish(&mut ledger, Action::RemoveScript(1));
        finish(&mut ledger, Action::RemoveScript(0));
        finish(&mut ledger, Action::RemoveBinding);
        assert_eq!(ledger.next(), Next::Retired);
    }

    #[test]
    fn cancellation_waits_for_each_late_registration_and_removes_all_known_scripts() {
        for count in 0..2 {
            let mut ledger = Ledger::new(2);
            finish(&mut ledger, Action::AddBinding);
            for index in 0..count {
                finish(&mut ledger, Action::AddScript(index));
            }
            assert!(ledger.begin(Action::AddScript(count)));
            ledger.close();
            assert_eq!(ledger.next(), Next::Await);
            assert!(!ledger.complete(Action::AddScript(1 - count), true));
            assert!(!ledger.complete(Action::RemoveBinding, true));
            assert!(ledger.complete(Action::AddScript(count), true));
            for index in (0..=count).rev() {
                finish(&mut ledger, Action::RemoveScript(index));
            }
            finish(&mut ledger, Action::RemoveBinding);
            assert_eq!(ledger.next(), Next::Retired);
        }
    }

    #[test]
    fn unknown_registration_retires_known_scripts_and_keeps_the_binding_uncertain() {
        for failed in 0..2 {
            let mut ledger = Ledger::new(2);
            finish(&mut ledger, Action::AddBinding);
            for index in 0..failed {
                finish(&mut ledger, Action::AddScript(index));
            }
            assert!(ledger.begin(Action::AddScript(failed)));
            assert!(ledger.complete(Action::AddScript(failed), false));
            for index in (0..failed).rev() {
                finish(&mut ledger, Action::RemoveScript(index));
            }
            assert_eq!(ledger.next(), Next::Uncertain);
            ledger.close();
            assert!(!ledger.begin(Action::RemoveBinding));
            assert_eq!(ledger.next(), Next::Uncertain);
        }
    }

    #[test]
    fn failed_removal_retries_the_exact_identifier_before_any_earlier_script_or_binding() {
        let mut ledger = Ledger::new(2);
        finish(&mut ledger, Action::AddBinding);
        finish(&mut ledger, Action::AddScript(0));
        finish(&mut ledger, Action::AddScript(1));
        ledger.close();
        for index in (0..2).rev() {
            let action = Action::RemoveScript(index);
            assert!(ledger.begin(action));
            assert!(ledger.complete(action, false));
            assert_eq!(ledger.next(), Next::Effect(action));
            assert!(!ledger.begin(Action::RemoveBinding));
            finish(&mut ledger, action);
        }
        assert!(ledger.begin(Action::RemoveBinding));
        assert!(ledger.complete(Action::RemoveBinding, false));
        finish(&mut ledger, Action::RemoveBinding);
        assert_eq!(ledger.next(), Next::Retired);
    }

    #[test]
    fn cancelling_binding_registration_waits_for_callback_and_checked_removal() {
        let mut ledger = Ledger::new(2);
        assert!(ledger.begin(Action::AddBinding));
        ledger.close();
        assert_eq!(ledger.next(), Next::Await);
        assert!(ledger.complete(Action::AddBinding, false));
        finish(&mut ledger, Action::RemoveBinding);
        assert_eq!(ledger.next(), Next::Retired);
    }
}
