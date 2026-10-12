// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Record provisional native references before any reentrant installation effect.

use crate::DesktopError;
use std::{cell::Cell, rc::Rc};

struct State<T> {
    callback: Rc<T>,
    native_reference: Cell<bool>,
    destroyed: Cell<bool>,
}

/// Retain exact callback metadata and actual final-destruction observation with its owner.
pub(crate) struct Registration<T>(Rc<State<T>>);
impl<T> Registration<T> {
    pub(crate) fn new(callback: &Rc<T>) -> Self {
        Self(Rc::new(State {
            callback: callback.clone(),
            native_reference: Cell::new(false),
            destroyed: Cell::new(false),
        }))
    }
    #[cfg(test)]
    pub(crate) fn reference_count(&self) -> usize {
        Rc::strong_count(&self.0)
    }
    pub(crate) fn installed(&self) -> bool {
        self.0.native_reference.get()
    }
    pub(crate) fn destroyed(&self) -> bool {
        self.0.destroyed.get()
    }
    pub(crate) fn data(&self) -> usize {
        Rc::as_ptr(&self.0) as usize
    }

    /// Drive the same provisional ownership boundary with native effects or deterministic fakes.
    pub(crate) fn install(
        &self,
        effect: impl FnOnce(usize) -> Result<(), DesktopError>,
    ) -> Result<(), DesktopError> {
        if self.installed() || self.destroyed() {
            return Err(DesktopError::message(
                "register_accessible_subclass",
                "registration is already installed or destroyed",
            ));
        }
        self.0.native_reference.set(true);
        let data = Rc::into_raw(self.0.clone()) as usize;
        let result = effect(data);
        if result.is_err() {
            // SAFETY: This owner remains live across effect/reentry. The flag identifies
            // whether the provisional native clone was already returned by final destruction.
            unsafe {
                reclaim::<T>(data, false);
            }
        }
        result
    }

    pub(crate) fn remove(
        &self,
        effect: impl FnOnce(usize) -> Result<(), DesktopError>,
    ) -> Result<(), DesktopError> {
        if self.destroyed() {
            return Ok(());
        }
        if !self.installed() {
            return Err(DesktopError::message(
                "remove_accessible_subclass",
                "no native registration is retained",
            ));
        }
        effect(self.data())?;
        // SAFETY: Only confirmed removal returns the exact clone, unless reentrant
        // final destruction already returned it during effect. Owner keeps State alive.
        unsafe {
            reclaim::<T>(self.data(), false);
        }
        Ok(())
    }
}

/// # Safety
/// `data` is the installed reference produced by Registration<T> for this procedure.
/// The concrete T must match installation; never retain this borrow across native forwarding.
pub(crate) unsafe fn callback<'a, T>(data: usize) -> &'a T {
    // SAFETY: The caller establishes this concrete registration/type and native lifetime.
    unsafe { &(*(data as *const State<T>)).callback }
}

/// # Safety
/// The exact owner/native State<T> remains live through the native procedure chain.
/// Called only after confirmed removal, failed provisional installation, or final destruction.
pub(crate) unsafe fn reclaim<T>(data: usize, destroyed: bool) {
    // SAFETY: Installation's separate owner keeps State alive while the flag is examined.
    let state = unsafe { &*(data as *const State<T>) };
    if destroyed {
        state.destroyed.set(true);
    }
    if state.native_reference.replace(false) {
        // SAFETY: The flag returns exactly this native Rc clone once.
        unsafe {
            drop(Rc::from_raw(data as *const State<T>));
        }
    }
}

#[cfg(test)]
#[path = "windows_registration_tests.rs"]
mod tests;
