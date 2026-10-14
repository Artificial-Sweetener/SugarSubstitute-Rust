// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Validate and retire owned process handles on the private native retirement thread.

use crate::native::{NativeCorrelation, NativeObservationError, ProcessExit, ProcessLease, Signal};
use ::windows::Win32::{
    Foundation::{
        CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, FILETIME, GetHandleInformation,
        HANDLE, HANDLE_FLAG_INHERIT,
    },
    System::Threading::{GetCurrentProcess, GetExitCodeProcess, GetProcessId, GetProcessTimes},
};
use std::{
    os::windows::io::{IntoRawHandle, OwnedHandle},
    sync::Arc,
};

use crate::native::wait;

// Integer handle identities permit moving unique ownership between threads.
// No borrowed HANDLE or callback pointer escapes this adapter.
pub(crate) struct Lease {
    original: Option<usize>,
    duplicate: Option<usize>,
    wait: Option<wait::Wait>,
    correlation: Option<NativeCorrelation>,
    pid: u32,
    creation: u64,
}
impl Lease {
    /// Complete custody observation peak including callback and distinct signal allocation.
    pub(crate) fn storage_bytes() -> usize {
        size_of::<Self>()
            + wait::Wait::storage_bytes()
            + size_of::<Signal>()
            + 2 * size_of::<usize>()
    }
    pub(crate) fn new(input: OwnedHandle) -> Self {
        Self {
            original: Some(input.into_raw_handle() as usize),
            duplicate: None,
            wait: None,
            correlation: None,
            pid: 0,
            creation: 0,
        }
    }
    fn process(&self) -> Result<HANDLE, NativeObservationError> {
        self.duplicate.map(|raw| HANDLE(raw as *mut _)).ok_or_else(|| {
            NativeObservationError::new(
                "process_identity",
                std::io::Error::other("Owned process duplicate is unavailable"),
            )
        })
    }
    fn close(
        handle: &mut Option<usize>,
        stage: &'static str,
    ) -> Result<(), NativeObservationError> {
        if let Some(raw) = *handle {
            // SAFETY: this unique owned identity remains unchanged on failure;
            // unregister has already established that no native waiter borrows it.
            unsafe { CloseHandle(HANDLE(raw as *mut _)) }
                .map_err(|cause| NativeObservationError::new(stage, cause))?;
            *handle = None;
        }
        Ok(())
    }
}
impl Lease {
    /// Check the closed original custody control before each fresh process effect.
    pub(crate) fn start_checked(
        &mut self,
        correlation: NativeCorrelation,
        signal: Arc<Signal>,
        expected: Option<(u32, u64)>,
        mut check: impl FnMut() -> Result<(), NativeObservationError>,
    ) -> Result<(), NativeObservationError> {
        self.correlation = Some(correlation);
        self.wait = Some(wait::Wait::new(signal));
        let original = self.original.ok_or_else(|| {
            NativeObservationError::new(
                "duplicate_process_handle",
                std::io::Error::other("Owned process input missing"),
            )
        })?;
        check()?;
        let mut duplicate = HANDLE::default();
        // SAFETY: current-process pseudo handles are borrowed, input is uniquely
        // owned, and output remains in this durable lease before later failures.
        let copied = unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                HANDLE(original as *mut _),
                GetCurrentProcess(),
                &mut duplicate,
                0,
                false,
                DUPLICATE_SAME_ACCESS,
            )
        };
        if !duplicate.is_invalid() {
            self.duplicate = Some(duplicate.0 as usize);
        }
        copied.map_err(|cause| NativeObservationError::new("duplicate_process_handle", cause))?;
        check()?;
        let process = self.process()?;
        let mut flags = 0;
        check()?;
        // SAFETY: duplicate is retained by this worker through all queries and retirement.
        unsafe { GetHandleInformation(process, &mut flags) }
            .map_err(|cause| NativeObservationError::new("process_handle_flags", cause))?;
        if flags & HANDLE_FLAG_INHERIT.0 != 0 {
            return Err(NativeObservationError::new(
                "process_handle_flags",
                std::io::Error::other("Process duplicate is inheritable"),
            ));
        }
        check()?;
        // SAFETY: wrong object types fail this query; original error is read immediately.
        self.pid = unsafe { GetProcessId(process) };
        if self.pid == 0 {
            return Err(NativeObservationError::new("process_id", std::io::Error::last_os_error()));
        }
        let (mut creation, mut exit, mut kernel, mut user) =
            (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        check()?;
        // SAFETY: all output FILETIMEs are owned stack values, handle remains alive.
        unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) }
            .map_err(|cause| NativeObservationError::new("process_creation_time", cause))?;
        self.creation =
            (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime);
        if self.creation == 0 {
            return Err(NativeObservationError::new(
                "process_creation_time",
                std::io::Error::other("Process creation timestamp is zero"),
            ));
        }
        if expected.is_some_and(|expected| expected != (self.pid, self.creation)) {
            return Err(NativeObservationError::new(
                "process_original_identity",
                std::io::Error::other("Process differs from its original native source"),
            ));
        }
        check()?;
        Self::close(&mut self.original, "close_process_input")?;
        check()?;
        match &mut self.wait {
            Some(wait) => wait.register(process),
            None => Err(NativeObservationError::new(
                "register_process_wait",
                std::io::Error::other("Wait owner missing"),
            )),
        }
    }
    /// Borrow actual validated kernel identity without another query.
    pub(crate) const fn identity(&self) -> (u32, u64) {
        (self.pid, self.creation)
    }
    /// Gate unregister and each close independently; adopt late successful returns first.
    pub(crate) fn retire_checked(
        &mut self,
        mut check: impl FnMut() -> Result<(), NativeObservationError>,
    ) -> Result<(), NativeObservationError> {
        if let Some(wait) = &mut self.wait {
            check()?;
            wait.retire()?;
        }
        if self.duplicate.is_some() {
            check()?;
            Self::close(&mut self.duplicate, "close_process_duplicate")?;
        }
        if self.original.is_some() {
            check()?;
            Self::close(&mut self.original, "close_process_input")?;
        }
        Ok(())
    }
}
impl ProcessLease for Lease {
    fn start(
        &mut self,
        correlation: NativeCorrelation,
        signal: Arc<Signal>,
    ) -> Result<(), NativeObservationError> {
        self.start_checked(correlation, signal, None, || Ok(()))
    }
    fn read_exit(&mut self) -> Result<ProcessExit, NativeObservationError> {
        if self.wait.as_ref().is_some_and(wait::Wait::timed_out) {
            return Err(NativeObservationError::new(
                "process_wait_signal",
                std::io::Error::other("Infinite process wait unexpectedly timed out"),
            ));
        }
        let mut exit_code = 0;
        // SAFETY: actual process signalling was observed, and duplicate is still owned.
        unsafe { GetExitCodeProcess(self.process()?, &mut exit_code) }
            .map_err(|cause| NativeObservationError::new("process_exit_code", cause))?;
        let correlation = self.correlation.ok_or_else(|| {
            NativeObservationError::new(
                "process_wait_signal",
                std::io::Error::other("Wait correlation missing"),
            )
        })?;
        Ok(ProcessExit {
            correlation,
            process_id: self.pid,
            creation_time_100ns: self.creation,
            exit_code,
        })
    }
    fn retire(&mut self) -> Result<(), NativeObservationError> {
        if let Some(wait) = &mut self.wait {
            wait.retire()?;
        }
        let mut failure = Self::close(&mut self.duplicate, "close_process_duplicate").err();
        if let Err(error) = Self::close(&mut self.original, "close_process_input") {
            if let Some(primary) = &mut failure {
                primary.retain_cleanup(error);
            } else {
                failure = Some(error);
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}
