// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt inherited originals and prepare only an exact four-event launch allowlist.
mod operations;
use crate::{
    fixture_forwarding::{FixtureForwardingEvents, FixtureForwardingPermit, ForwardLease},
    fixture_session::child::InheritedFixtureEvents,
    native::NativeObservationError,
};
use operations::Operations;
use std::os::windows::io::BorrowedHandle;
use windows::Win32::{
    Foundation::{
        CloseHandle, DuplicateHandle, GetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT,
        SetHandleInformation,
    },
    System::Threading::{
        EVENT_MODIFY_STATE, GetCurrentProcess, ResetEvent, SYNCHRONIZATION_SYNCHRONIZE,
    },
};
impl FixtureForwardingPermit {
    /// Adopt exactly four exclusively inherited handles after successful reservation.
    /// Cancellation before bind still adopts and closes them on the native worker.
    pub fn bind(
        self,
        events: InheritedFixtureEvents,
    ) -> crate::fixture_forwarding::FixtureForwardingSession {
        self.bind_lease(Box::new(Lease {
            originals: events.handles.map(Some),
            duplicates: [None; 4],
        }))
    }
}
impl FixtureForwardingEvents {
    /// Borrow the exact restricted duplicates while this noncloneable pin retains closure.
    /// Keep the pin through native CreateProcess and DeleteProcThreadAttributeList.
    pub fn borrow_inheritance(&self) -> crate::windows_fixture::FixtureInheritance<'_> {
        let [ready, release, challenge, response] = self.handles();
        // SAFETY: the pin prevents every duplicate from closing until its own Drop.
        unsafe {
            crate::windows_fixture::FixtureInheritance {
                ready: BorrowedHandle::borrow_raw(ready as *mut _),
                release: BorrowedHandle::borrow_raw(release as *mut _),
                challenge: BorrowedHandle::borrow_raw(challenge as *mut _),
                response: BorrowedHandle::borrow_raw(response as *mut _),
            }
        }
    }
}
struct Lease {
    originals: [Option<usize>; 4],
    duplicates: [Option<usize>; 4],
}
fn handle(raw: Option<usize>) -> Result<HANDLE, NativeObservationError> {
    raw.map(|raw| HANDLE(raw as *mut _)).ok_or_else(|| {
        NativeObservationError::new("forwarding_handle", std::io::Error::other("Handle missing"))
    })
}
impl ForwardLease for Lease {
    fn prepare(&mut self) -> Result<[usize; 4], NativeObservationError> {
        self.prepare_with(&mut operations::Native)
    }
    fn retire(&mut self) -> Result<(), NativeObservationError> {
        self.retire_with(&mut operations::Native)
    }
}
impl Lease {
    fn prepare_with(
        &mut self,
        operations: &mut impl Operations,
    ) -> Result<[usize; 4], NativeObservationError> {
        for index in 0..4 {
            let source = handle(self.originals[index])?.0 as usize;
            let flags = operations.flags(source)?;
            if flags & HANDLE_FLAG_INHERIT.0 == 0 {
                return Err(NativeObservationError::new(
                    "forwarding_original_flags",
                    std::io::Error::other("Expected inherited event"),
                ));
            }
            operations.clear_inherit(source)?;
            let flags = operations.flags(source)?;
            if flags & HANDLE_FLAG_INHERIT.0 != 0 {
                return Err(NativeObservationError::new(
                    "forwarding_verify_inherit",
                    std::io::Error::other("Inherit flag remains set"),
                ));
            }
            operations.validate_event(source)?;
            let mut duplicate = 0;
            let copied = operations.duplicate(source, &mut duplicate);
            if duplicate != 0 {
                self.duplicates[index] = Some(duplicate);
            }
            copied?;
            let flags = operations.flags(handle(self.duplicates[index])?.0 as usize)?;
            if flags & HANDLE_FLAG_INHERIT.0 == 0 {
                return Err(NativeObservationError::new(
                    "forwarding_duplicate_flags",
                    std::io::Error::other("Duplicate is not inheritable"),
                ));
            }
        }
        Ok([
            handle(self.duplicates[0])?.0 as usize,
            handle(self.duplicates[1])?.0 as usize,
            handle(self.duplicates[2])?.0 as usize,
            handle(self.duplicates[3])?.0 as usize,
        ])
    }
    fn retire_with(
        &mut self,
        operations: &mut impl Operations,
    ) -> Result<(), NativeObservationError> {
        let mut failure: Option<NativeObservationError> = None;
        for slot in self.duplicates.iter_mut().chain(self.originals.iter_mut()) {
            if let Some(raw) = *slot {
                match operations.close(raw) {
                    Ok(()) => *slot = None,
                    Err(error) => {
                        if let Some(first) = &mut failure {
                            first.retain_cleanup(error);
                        } else {
                            failure = Some(error);
                        }
                    }
                }
            }
        }
        failure.map_or(Ok(()), Err)
    }
}
#[cfg(test)]
mod native_abi_tests;
#[cfg(test)]
mod tests;
