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
                APTTYPE, APTTYPE_CURRENT, APTTYPE_MAINSTA, APTTYPE_MTA, APTTYPE_STA,
                APTTYPEQUALIFIER, APTTYPEQUALIFIER_IMPLICIT_MTA, APTTYPEQUALIFIER_NONE,
                COINIT_APARTMENTTHREADED, CoGetApartmentType, CoInitializeEx, CoUninitialize,
            },
            Threading::GetCurrentThreadId,
        },
    },
    core::Error,
};

/// Preserve both native facts so inherited MTA state cannot impersonate explicit initialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Apartment {
    kind: APTTYPE,
    qualifier: APTTYPEQUALIFIER,
}

trait Boundary {
    fn query(&mut self) -> Result<Apartment, Error>;
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
        let acquire = match boundary.query() {
            Ok(Apartment { kind, qualifier })
                if (kind == APTTYPE_STA || kind == APTTYPE_MAINSTA)
                    && qualifier == APTTYPEQUALIFIER_NONE =>
            {
                false
            }
            Ok(Apartment { kind: APTTYPE_MTA, qualifier: APTTYPEQUALIFIER_IMPLICIT_MTA }) => true,
            Ok(_) => {
                return Err(DesktopError::new(
                    "ui_apartment_query",
                    Error::from(RPC_E_CHANGED_MODE),
                ));
            }
            Err(cause) if cause.code() == CO_E_NOTINITIALIZED => true,
            Err(cause) => return Err(DesktopError::new("ui_apartment_query", cause)),
        };
        let ownership = if acquire {
            boundary.acquire().map_err(|cause| DesktopError::new("ui_apartment_acquire", cause))?;
            Ownership::Acquired
        } else {
            Ownership::Borrowed
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
    fn query(&mut self) -> Result<Apartment, Error> {
        let mut kind = APTTYPE_CURRENT;
        let mut qualifier = APTTYPEQUALIFIER_NONE;
        // SAFETY: Valid writable scalar outputs; the UI caller owns this current thread.
        unsafe {
            CoGetApartmentType(&mut kind, &mut qualifier)?;
        }
        Ok(Apartment { kind, qualifier })
    }
    fn acquire(&mut self) -> Result<(), Error> {
        // SAFETY: Query proved no explicit initialization; acquire one reference on this UI thread.
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
    /// Borrow an ordinary STA or acquire one reference for an uninitialized or implicit-MTA thread.
    /// Preserve query/acquisition causes; reject explicit MTA, neutral and unsupported qualifiers.
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
