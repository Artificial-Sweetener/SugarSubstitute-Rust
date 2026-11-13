// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate inherited event roles before readiness; keep all outputs through retirement.
use crate::{
    fixture_session::child::{ChildLease, InheritedFixtureEvents},
    native::{NativeObservationError, Signal, wait},
};
use std::sync::Arc;
use windows::Win32::{
    Foundation::{
        CloseHandle, GetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation,
    },
    System::Threading::{ResetEvent, SetEvent},
};

pub(super) struct Lease {
    handles: [Option<usize>; 4],
    waits: [Option<wait::Wait>; 2],
}
impl Lease {
    pub(super) fn new(events: InheritedFixtureEvents) -> Self {
        Self { handles: events.handles.map(Some), waits: [None, None] }
    }
    fn handle(&self, index: usize) -> Result<HANDLE, NativeObservationError> {
        self.handles[index].map(|raw| HANDLE(raw as *mut _)).ok_or_else(|| {
            NativeObservationError::new(
                "fixture_child_handle",
                std::io::Error::other("Inherited event missing"),
            )
        })
    }
}
impl ChildLease for Lease {
    fn start(
        &mut self,
        challenge: Arc<Signal>,
        release: Arc<Signal>,
    ) -> Result<(), NativeObservationError> {
        for index in 0..4 {
            let handle = self.handle(index)?;
            let mut flags = 0;
            // SAFETY: bind transfers exclusive current-process inherited ownership.
            // All four slots stay retained until validation and checked closure.
            unsafe { GetHandleInformation(handle, &mut flags) }.map_err(|cause| {
                NativeObservationError::new("fixture_child_inherited_flags", cause)
            })?;
            if flags & HANDLE_FLAG_INHERIT.0 == 0 {
                return Err(NativeObservationError::new(
                    "fixture_child_inherited_flags",
                    std::io::Error::other("Expected an inherited fixture event"),
                ));
            }
            // SAFETY: retained current-process handle; change only its inherit bit.
            unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT.0, Default::default()) }
                .map_err(|cause| {
                    NativeObservationError::new("fixture_child_clear_inherit", cause)
                })?;
            // SAFETY: handle is still owned; flags is live writable storage.
            unsafe { GetHandleInformation(handle, &mut flags) }.map_err(|cause| {
                NativeObservationError::new("fixture_child_verify_inherit", cause)
            })?;
            if flags & HANDLE_FLAG_INHERIT.0 != 0 {
                return Err(NativeObservationError::new(
                    "fixture_child_verify_inherit",
                    std::io::Error::other("Fixture inherit flag remains set"),
                ));
            }
            // SAFETY: Reset validates event type/modification rights and sets false. Parent
            // source created false; no Challenge/Release is legal before Ready.
            unsafe { ResetEvent(handle) }.map_err(|cause| {
                NativeObservationError::new("fixture_child_validate_event", cause)
            })?;
        }
        self.waits[0] = Some(wait::Wait::for_operation(
            challenge,
            "register_fixture_challenge",
            "unregister_fixture_challenge",
        ));
        self.waits[1] = Some(wait::Wait::for_operation(
            release,
            "register_fixture_release",
            "unregister_fixture_release",
        ));
        for (slot, role) in [(0, 2), (1, 1)] {
            let handle = self.handle(role)?;
            if let Some(wait) = &mut self.waits[slot] {
                wait.register(handle)?;
            }
        }
        Ok(())
    }
    fn ready(&mut self) -> Result<(), NativeObservationError> {
        // SAFETY: retained validated Ready event, after both registrations exist.
        unsafe { SetEvent(self.handle(0)?) }
            .map_err(|cause| NativeObservationError::new("signal_fixture_ready", cause))
    }
    fn response(&mut self) -> Result<(), NativeObservationError> {
        // SAFETY: retained validated Response event; worker observed Challenge.
        unsafe { SetEvent(self.handle(3)?) }
            .map_err(|cause| NativeObservationError::new("signal_fixture_response", cause))
    }
    fn validate_observation(&self) -> Result<(), NativeObservationError> {
        if self.waits.iter().flatten().any(wait::Wait::timed_out) {
            Err(NativeObservationError::new(
                "fixture_child_wait_timeout",
                std::io::Error::other("Infinite fixture wait reported timeout"),
            ))
        } else {
            Ok(())
        }
    }
    fn retire(&mut self) -> Result<(), NativeObservationError> {
        let mut first: Option<NativeObservationError> = None;
        for wait in self.waits.iter_mut().flatten() {
            if let Err(error) = wait.retire() {
                retain(&mut first, error);
            }
        }
        if let Some(error) = first {
            return Err(error);
        }
        for slot in &mut self.handles {
            if let Some(raw) = *slot {
                // SAFETY: unique transferred handle, after every callback quiesced.
                match unsafe { CloseHandle(HANDLE(raw as *mut _)) } {
                    Ok(()) => *slot = None,
                    Err(cause) => retain(
                        &mut first,
                        NativeObservationError::new("close_fixture_child_event", cause),
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
