// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Share an actual completion on the native thread under its original reservation.
use super::*;
use std::{fmt, rc::Rc};
struct Control(Option<Rc<MemoryClaim>>);
impl Clone for Control {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for Control {
    fn drop(&mut self) {
        if let Some(control) = self.0.take() {
            drop(Rc::into_inner(control));
        }
    }
}
pub(super) struct MainTerminalOwner {
    value: Option<Rc<MainTerminal>>,
    control: Control,
}
impl MainTerminalOwner {
    pub(super) fn new(slot: &mut Option<MainTerminal>) -> Result<Self> {
        let completion = slot.as_ref().ok_or("Actual main terminal absent")?;
        let bytes = (size_of::<MainTerminal>()
            + size_of::<MemoryClaim>()
            + 4 * size_of::<usize>()
            + 2 * size_of::<Self>()) as u64;
        let control = completion.memory.claim(bytes)?;
        let value = slot.take().ok_or("Actual main terminal changed")?;
        Ok(Self { value: Some(Rc::new(value)), control: Control(Some(Rc::new(control))) })
    }
    pub(super) fn get(&self) -> Option<&MainTerminal> {
        self.value.as_deref()
    }
    pub(super) fn get_mut(&mut self) -> Option<&mut MainTerminal> {
        self.value.as_mut().and_then(Rc::get_mut)
    }
}
impl Clone for MainTerminalOwner {
    fn clone(&self) -> Self {
        Self { value: self.value.clone(), control: self.control.clone() }
    }
}
impl Drop for MainTerminalOwner {
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            drop(Rc::into_inner(value));
        }
    }
}
/// The returned error borrows original causes through the same retained completion.
pub(in crate::production) struct MainTerminalIssue(pub(super) MainTerminalOwner);
impl MainTerminalIssue {
    /// Borrow the actual supervised panic rather than copying or formatting its text automatically.
    pub(in crate::production) fn panic_cause(&self) -> Option<&PanicCause> {
        match &self.0.get()?.outcome {
            JobOutcome::Panicked(cause) => Some(cause),
            _ => None,
        }
    }
}
impl fmt::Debug for MainTerminalIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MainTerminalIssue")
            .field("kind", &self.0.get().map(|done| done.outcome.kind()))
            .field("panic_retained", &self.panic_cause().is_some())
            .finish()
    }
}
impl fmt::Display for MainTerminalIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Main request ended with {:?}", self.0.get().map(|done| done.outcome.kind()))
    }
}
impl Error for MainTerminalIssue {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.0.get()?.outcome {
            JobOutcome::Failure(cause) => Some(cause),
            _ => None,
        }
    }
}
