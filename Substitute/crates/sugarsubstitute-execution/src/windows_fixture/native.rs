// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain all partial native outputs until callback quiescence and checked closure.
use crate::native::wait;
use crate::{
    fixture_session::ParentLease,
    native::{NativeObservationError, Signal},
};
use std::sync::Arc;
use windows::Win32::{
    Foundation::{CloseHandle, DuplicateHandle, GetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT},
    System::Threading::{
        CreateEventW, EVENT_MODIFY_STATE, GetCurrentProcess, SYNCHRONIZATION_SYNCHRONIZE, SetEvent,
    },
};
pub(super) struct Lease {
    events: [Option<usize>; 4],
    duplicates: [Option<usize>; 4],
    waits: [Option<wait::Wait>; 2],
}
impl Lease {
    pub(super) fn new() -> Self {
        Self { events: [None; 4], duplicates: [None; 4], waits: [None, None] }
    }
}
fn event(raw: Option<usize>, stage: &'static str) -> Result<HANDLE, NativeObservationError> {
    raw.map(|raw| HANDLE(raw as *mut _)).ok_or_else(|| {
        NativeObservationError::new(stage, std::io::Error::other("Owned fixture handle missing"))
    })
}
impl ParentLease for Lease {
    fn start(
        &mut self,
        ready: Arc<Signal>,
        response: Arc<Signal>,
    ) -> Result<[usize; 4], NativeObservationError> {
        for index in 0..4 {
            // SAFETY: fresh unnamed manual-reset events are noninheritable and initially false.
            let handle = unsafe { CreateEventW(None, true, false, None) }
                .map_err(|cause| NativeObservationError::new("create_fixture_event", cause))?;
            self.events[index] = Some(handle.0 as usize);
            let mut duplicate = HANDLE::default();
            // SAFETY: owned real event source; adopt any output before inspecting failure.
            let result = unsafe {
                DuplicateHandle(
                    GetCurrentProcess(),
                    handle,
                    GetCurrentProcess(),
                    &mut duplicate,
                    EVENT_MODIFY_STATE.0 | SYNCHRONIZATION_SYNCHRONIZE.0,
                    true,
                    Default::default(),
                )
            };
            if !duplicate.is_invalid() {
                self.duplicates[index] = Some(duplicate.0 as usize);
            }
            result
                .map_err(|cause| NativeObservationError::new("duplicate_fixture_event", cause))?;
            let mut flags = 0;
            // SAFETY: checked retained duplicate; flags is live writable storage.
            unsafe {
                GetHandleInformation(
                    event(self.duplicates[index], "fixture_duplicate")?,
                    &mut flags,
                )
            }
            .map_err(|cause| NativeObservationError::new("fixture_duplicate_flags", cause))?;
            if flags & HANDLE_FLAG_INHERIT.0 == 0 {
                return Err(NativeObservationError::new(
                    "fixture_duplicate_flags",
                    std::io::Error::other("Fixture duplicate is not inheritable"),
                ));
            }
        }
        self.waits[0] = Some(wait::Wait::for_operation(
            ready,
            "register_fixture_ready",
            "unregister_fixture_ready",
        ));
        self.waits[1] = Some(wait::Wait::for_operation(
            response,
            "register_fixture_response",
            "unregister_fixture_response",
        ));
        for (slot, role) in [(0, 0), (1, 3)] {
            if let Some(wait) = &mut self.waits[slot] {
                wait.register(event(self.events[role], "fixture_wait_event")?)?;
            }
        }
        Ok([
            event(self.duplicates[0], "fixture_ready_duplicate")?.0 as usize,
            event(self.duplicates[1], "fixture_release_duplicate")?.0 as usize,
            event(self.duplicates[2], "fixture_challenge_duplicate")?.0 as usize,
            event(self.duplicates[3], "fixture_response_duplicate")?.0 as usize,
        ])
    }
    fn challenge(&mut self) -> Result<(), NativeObservationError> {
        // SAFETY: worker owns the validated Challenge event through this call.
        unsafe { SetEvent(event(self.events[2], "fixture_challenge")?) }
            .map_err(|cause| NativeObservationError::new("signal_fixture_challenge", cause))
    }
    fn validate_observation(&self) -> Result<(), NativeObservationError> {
        if self.waits.iter().flatten().any(wait::Wait::timed_out) {
            Err(NativeObservationError::new(
                "fixture_wait_timeout",
                std::io::Error::other("Infinite fixture wait reported timeout"),
            ))
        } else {
            Ok(())
        }
    }
    fn release(&mut self) -> Result<(), NativeObservationError> {
        // SAFETY: worker owns the validated Release event through this call.
        unsafe { SetEvent(event(self.events[1], "fixture_release")?) }
            .map_err(|cause| NativeObservationError::new("signal_fixture_release", cause))
    }
    fn retire(&mut self) -> Result<(), NativeObservationError> {
        let mut first = None;
        for wait in self.waits.iter_mut().flatten() {
            if let Err(error) = wait.retire() {
                retain(&mut first, error);
            }
        }
        if let Some(error) = first {
            return Err(error);
        }
        for slot in self.duplicates.iter_mut().chain(self.events.iter_mut()) {
            if let Some(raw) = *slot {
                // SAFETY: no outstanding callback or launch pin references this owned slot.
                match unsafe { CloseHandle(HANDLE(raw as *mut _)) } {
                    Ok(()) => *slot = None,
                    Err(cause) => retain(
                        &mut first,
                        NativeObservationError::new("close_fixture_event", cause),
                    ),
                }
            }
        }
        first.map_or(Ok(()), Err)
    }
}
fn retain(first: &mut Option<NativeObservationError>, error: NativeObservationError) {
    match first {
        Some(first) => first.retain_cleanup(error),
        None => *first = Some(error),
    }
}
