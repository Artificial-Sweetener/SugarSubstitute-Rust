// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Continuously drain exactly owned stdout/stderr through the central native owner.
#[cfg(any(test, feature = "boundary-fixtures"))]
pub use crate::output::boundary_fixture;
pub use crate::output::{
    AdmittedOutputBatch, DesktopOutputBatch, MAX_WINDOWS_OUTPUT_CHUNKS,
    MAX_WINDOWS_OUTPUT_QUEUED_BYTES, MIN_WINDOWS_OUTPUT_BYTES, OutputIdentity, WindowsChildStdio,
    WindowsOutputCancellation, WindowsOutputControl, WindowsOutputCounters, WindowsOutputError,
    WindowsOutputReport, WindowsOutputSession,
};
use crate::{ExecutionClient, output};
use std::os::windows::io::BorrowedHandle;
use sugarsubstitute_contracts::execution::{AdmissionError, JobSpec, Scope};
#[allow(unsafe_code)]
#[path = "windows_output/native.rs"]
mod native;
#[cfg(test)]
#[allow(unsafe_code)]
#[path = "windows_output/native_abi_tests.rs"]
mod native_abi_tests;
/// Borrow the exact allowlist through CreateProcess and actual attribute deletion.
pub struct WindowsOutputInheritance<'a> {
    /// Read-only NUL input, inherited only by this owned launch.
    pub stdin: BorrowedHandle<'a>,
    /// Synchronous writable child stdout end; parent reading is overlapped.
    pub stdout: BorrowedHandle<'a>,
    /// Separate synchronous writable stderr end.
    pub stderr: BorrowedHandle<'a>,
}
impl WindowsChildStdio {
    /// Reject foreign/stale identity before exposing pinned native launch handles.
    /// Identity is a trusted composing-owner fact, not authentication evidence.
    #[allow(unsafe_code)]
    pub fn borrow_inheritance(
        &self,
        expected: OutputIdentity,
    ) -> Result<WindowsOutputInheritance<'_>, WindowsOutputError> {
        let [stdin, stdout, stderr] = self.handles(expected)?;
        // SAFETY: this noncloneable live pin prevents checked closure throughout
        // every borrow; only the worker ever owns/closes these original handles.
        unsafe {
            Ok(WindowsOutputInheritance {
                stdin: BorrowedHandle::borrow_raw(stdin as *mut _),
                stdout: BorrowedHandle::borrow_raw(stdout as *mut _),
                stderr: BorrowedHandle::borrow_raw(stderr as *mut _),
            })
        }
    }
}
impl ExecutionClient {
    /// Admit all output storage before any native pipe/event/NUL creation.
    /// Keep scope/control with the identity, independently of splash/console views.
    /// Use a native scope whose lifetime can outlive launch cancellation and cleanup.
    /// EOF is an actual per-stream fact, never root exit or family settlement.
    pub fn reserve_windows_output(
        &self,
        scope: &Scope,
        spec: JobSpec,
        identity: OutputIdentity,
    ) -> Result<WindowsOutputSession, AdmissionError> {
        output::reserve(&self.core, scope, spec, identity, Box::new(native::Lease::new()))
    }
}
