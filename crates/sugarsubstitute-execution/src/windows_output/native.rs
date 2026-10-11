// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own pipe ends and checked OVERLAPPED teardown on the sole native worker.
use crate::{
    native::{NativeObservationError, Signal},
    output::*,
};
use std::sync::Arc;
use sugarsubstitute_contracts::{
    lifecycle::ResourceKey,
    transcript::{MAX_BATCH_BYTES, OutputStream},
};
use windows::{
    Win32::{
        Foundation::*,
        Security::SECURITY_ATTRIBUTES,
        Storage::FileSystem::*,
        System::{IO::*, Pipes::*, Threading::*},
    },
    core::PCWSTR,
};
#[path = "operation.rs"]
mod operation;
#[path = "prepare.rs"]
mod prepare;
pub(super) struct Lease {
    read: [Handle; 2],
    child: [Handle; 3],
    operations: [Option<operation::Operation>; 2],
}
pub(super) struct Handle(usize);
impl Handle {
    fn raw(&self) -> HANDLE {
        HANDLE(self.0 as *mut _)
    }
    fn close(&mut self, stage: &'static str) -> Result<(), NativeObservationError> {
        if self.0 != 0 {
            // SAFETY: the unique retained slot remains unchanged on checked failure.
            unsafe { CloseHandle(self.raw()) }.map_err(|e| error(stage, e))?;
            self.0 = 0;
        }
        Ok(())
    }
    fn adopt(&mut self, value: HANDLE, stage: &'static str) -> Result<(), NativeObservationError> {
        if value.is_invalid() {
            return Err(error(stage, last_error()));
        }
        self.0 = value.0 as usize;
        Ok(())
    }
}
fn last_error() -> windows::core::Error {
    // SAFETY: capture this thread's immediate original error code.
    let code = unsafe { GetLastError() }.0;
    windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(if code == 0 {
        ERROR_GEN_FAILURE.0
    } else {
        code
    }))
}
fn code(e: &windows::core::Error, expected: WIN32_ERROR) -> bool {
    e.code() == windows::core::HRESULT::from_win32(expected.0)
}
impl Lease {
    pub(super) fn new() -> Self {
        Self {
            read: [Handle(0), Handle(0)],
            child: [Handle(0), Handle(0), Handle(0)],
            operations: [None, None],
        }
    }
    fn op(
        &mut self,
        stream: OutputStream,
    ) -> Result<&mut operation::Operation, NativeObservationError> {
        self.operations[stream.index()].as_mut().ok_or_else(|| invariant("output_operation_owner"))
    }
}
impl OutputLease for Lease {
    fn prepare(
        &mut self,
        resource: ResourceKey,
        signals: [Arc<Signal>; 2],
    ) -> Result<[usize; 3], NativeObservationError> {
        for (index, signal) in signals.into_iter().enumerate() {
            self.operations[index] = Some(operation::Operation::new(signal));
        }
        self.prepare_handles(resource)?;
        Ok(self.child.each_ref().map(|h| h.0))
    }
    fn release_child_ends(&mut self) -> Result<(), NativeObservationError> {
        for child in &mut self.child {
            child.close("output_close_parent_child_end")?;
        }
        Ok(())
    }
    fn read(
        &mut self,
        stream: OutputStream,
        bytes: &mut [u8],
    ) -> Result<Progress, NativeObservationError> {
        let pipe = self.read[stream.index()].raw();
        self.op(stream)?.read(pipe, bytes)
    }
    fn poll(
        &mut self,
        stream: OutputStream,
        _bytes: &mut [u8],
    ) -> Result<Progress, NativeObservationError> {
        let pipe = self.read[stream.index()].raw();
        self.op(stream)?.poll(pipe)
    }
    fn quiesce(&mut self, stream: OutputStream) -> Result<(), NativeObservationError> {
        self.op(stream)?.quiesce()
    }
    fn take_completed_read(&mut self, stream: OutputStream) -> Option<Progress> {
        self.operations[stream.index()].as_mut().and_then(|op| op.take_completed())
    }
    fn retire(&mut self, pinned: bool) -> Result<bool, NativeObservationError> {
        let mut settled = true;
        for index in 0..2 {
            if let Some(operation) = &mut self.operations[index] {
                settled &= operation.retire(self.read[index].raw())?;
            }
        }
        if !settled || pinned {
            return Ok(false);
        }
        self.release_child_ends()?;
        for read in &mut self.read {
            read.close("output_close_read_pipe")?;
        }
        Ok(true)
    }
}
