// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one outer UI STA obligation across native windows and every COM receiver.

use crate::DesktopError;
use std::{marker::PhantomData, rc::Rc};
use windows::{
    Win32::{
        Foundation::{CO_E_NOTINITIALIZED, RPC_E_CHANGED_MODE, RPC_E_WRONG_THREAD},
        System::{
            Com::{
                APTTYPE, APTTYPE_CURRENT, APTTYPE_MAINSTA, APTTYPE_STA, APTTYPEQUALIFIER_NONE,
                COINIT_APARTMENTTHREADED, CoGetApartmentType, CoInitializeEx, CoUninitialize,
            },
            Threading::GetCurrentThreadId,
        },
    },
    core::Error,
};

trait Boundary {
    fn query(&mut self) -> Result<APTTYPE, Error>;
    fn acquire(&mut self) -> Result<(), Error>;
    fn release(&mut self) -> Result<(), Error>;
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Ownership {
    Borrowed,
    Acquired,
    Released,
}

struct Owner<B: Boundary> {
    boundary: B,
    ownership: Ownership,
    affinity: PhantomData<Rc<()>>,
}
impl<B: Boundary> Owner<B> {
    fn enter(mut boundary: B) -> Result<Self, DesktopError> {
        let ownership = match boundary.query() {
            Ok(kind) if kind == APTTYPE_STA || kind == APTTYPE_MAINSTA => Ownership::Borrowed,
            Ok(_) => {
                return Err(DesktopError::new(
                    "ui_apartment_query",
                    Error::from(RPC_E_CHANGED_MODE),
                ));
            }
            Err(cause) if cause.code() == CO_E_NOTINITIALIZED => {
                boundary
                    .acquire()
                    .map_err(|cause| DesktopError::new("ui_apartment_acquire", cause))?;
                Ownership::Acquired
            }
            Err(cause) => return Err(DesktopError::new("ui_apartment_query", cause)),
        };
        Ok(Self { boundary, ownership, affinity: PhantomData })
    }
    fn release(&mut self) -> Result<(), DesktopError> {
        if self.ownership == Ownership::Acquired {
            self.boundary
                .release()
                .map_err(|cause| DesktopError::new("ui_apartment_release", cause))?;
        }
        self.ownership = Ownership::Released;
        Ok(())
    }
}
impl<B: Boundary> Drop for Owner<B> {
    fn drop(&mut self) {
        if self.ownership == Ownership::Acquired {
            tracing::error!(
                operation = "ui_apartment_unresolved_drop",
                "retaining owned STA reference until thread termination"
            );
        }
    }
}

struct NativeBoundary {
    thread: u32,
}
impl Boundary for NativeBoundary {
    fn query(&mut self) -> Result<APTTYPE, Error> {
        let mut kind = APTTYPE_CURRENT;
        let mut qualifier = APTTYPEQUALIFIER_NONE;
        // SAFETY: Valid writable scalar outputs; the UI caller owns this current thread.
        unsafe {
            CoGetApartmentType(&mut kind, &mut qualifier)?;
        }
        Ok(kind)
    }
    fn acquire(&mut self) -> Result<(), Error> {
        // SAFETY: Query proved no apartment; acquire exactly one reference on this UI thread.
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }
    }
    fn release(&mut self) -> Result<(), Error> {
        // SAFETY: GetCurrentThreadId has no input pointer or ownership effect.
        if unsafe { GetCurrentThreadId() } != self.thread {
            return Err(Error::from(RPC_E_WRONG_THREAD));
        }
        // SAFETY: The outer owner calls this only after all native/COM teardown; it acquired one reference.
        unsafe {
            CoUninitialize();
        }
        Ok(())
    }
}

/// Keep the current UI STA alive through every actual native resource/reference retirement.
/// This owner is not Send; dropping it performs no native cleanup and grants no settlement proof.
pub(super) struct UiApartment(Owner<NativeBoundary>);
impl UiApartment {
    /// Borrow a proven existing STA or acquire one reference when the thread is uninitialized.
    /// Preserve query/acquisition causes and reject MTA before any Wry effects.
    pub(super) fn enter() -> Result<Self, DesktopError> {
        // SAFETY: Scalar current-thread observation; no retained kernel handle is created.
        let thread = unsafe { GetCurrentThreadId() };
        Owner::enter(NativeBoundary { thread }).map(Self)
    }
    /// Balance only this owner's acquired reference, once, after actual full teardown.
    ///
    /// # Safety
    /// All construction/view/CDP receiver interfaces, callbacks, parent windows and native
    /// registrations must have actually retired and been released on this same UI thread.
    /// On uncertain retirement retain this owner and every parent/native resource instead.
    pub(super) unsafe fn release_after_teardown(&mut self) -> Result<(), DesktopError> {
        self.0.release()
    }
}

#[cfg(test)]
#[path = "windows_ui_apartment_tests.rs"]
mod tests;
