// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Issue the fixed Crashpad ping through the sole retained native pipe operation.
use crate::{
    capture_attach::Shared,
    native::NativeObservationError,
    native_pipe::{Progress, overlapped},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::JobContext;
use windows::Win32::Foundation::HANDLE;
pub(super) struct Ping {
    input: Box<[u8; 36]>,
    output: Box<[u8; 12]>,
    operation: overlapped::Operation,
    initial: Option<Progress>,
    issued: bool,
}
impl Ping {
    pub(super) fn new(signal: Arc<crate::native::Signal>) -> Self {
        let mut input = Box::new([0; 36]);
        input[..4].copy_from_slice(&2u32.to_le_bytes());
        Self {
            input,
            output: Box::new([0; 12]),
            operation: overlapped::Operation::new(signal),
            initial: None,
            issued: false,
        }
    }
    pub(super) fn issue(
        &mut self,
        pipe: HANDLE,
        shared: &Arc<Shared>,
        context: &JobContext,
    ) -> Result<(), NativeObservationError> {
        self.operation.prepare().map_err(|cause| cause.cause)?;
        let dispatch = crate::capture_attach::issue::execute(shared, context, || {
            self.operation
                .transact(pipe, self.input.as_ref(), self.output.as_mut())
                .map_err(|cause| cause.cause)
        })?;
        let accepted = match dispatch {
            crate::capture_attach::issue::Dispatch::Issued(accepted) => accepted,
            crate::capture_attach::issue::Dispatch::Stopped
            | crate::capture_attach::issue::Dispatch::Expired => return Ok(()),
        };
        self.issued = true;
        self.initial =
            Some(self.operation.initial_progress(accepted, pipe).map_err(|cause| cause.cause)?);
        Ok(())
    }
    pub(super) fn progress(&mut self, pipe: HANDLE) -> Result<bool, NativeObservationError> {
        if !self.issued {
            return Err(invariant("capture_ping_without_issue"));
        }
        let progress = match self.initial.take() {
            Some(progress) => progress,
            None => self.operation.poll(pipe).map_err(|cause| cause.cause)?,
        };
        match progress {
            Progress::Pending => Ok(false),
            Progress::Complete(length) => {
                self.operation.quiesce().map_err(|cause| cause.cause)?;
                if length != 12 || self.output.iter().any(|byte| *byte != 0) {
                    return Err(invariant("capture_ping_response"));
                }
                Ok(true)
            }
        }
    }
    pub(super) fn retire(&mut self, pipe: HANDLE) -> Result<bool, NativeObservationError> {
        self.operation.retire(pipe).map_err(|cause| cause.cause)
    }
}
pub(super) fn invariant(stage: &'static str) -> NativeObservationError {
    NativeObservationError::new(
        stage,
        std::io::Error::other("Capture native custody invariant rejected"),
    )
}
