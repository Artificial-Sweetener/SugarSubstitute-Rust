// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the whole physical port and its outside allocation inseparable through teardown.
use super::*;

pub(crate) struct OwnedPort {
    native: Option<Box<dyn Port>>,
    _allocation: Option<allocation::DiagnosticNativeOwnerLoan>,
}
impl OwnedPort {
    pub(crate) fn new(
        native: Box<dyn Port>,
        allocation: Option<allocation::DiagnosticNativeOwnerLoan>,
    ) -> Self {
        Self { native: Some(native), _allocation: allocation }
    }
    pub(super) fn borrow(
        &mut self,
        key: Option<LogicalStoreOperationKey>,
    ) -> Result<&mut (dyn Port + '_), DiagnosticStorageError> {
        match self.native.as_mut() {
            Some(native) => Ok(native.as_mut()),
            None => Err(DiagnosticStorageError::rejected(key, Code::Closed)),
        }
    }
    pub(super) fn live_handles(&self) -> usize {
        self.native.as_ref().map_or(0, |native| native.live_handles())
    }
    pub(super) fn retained_files(&self) -> usize {
        self.native.as_ref().map_or(0, |native| native.retained_files())
    }
}
impl Drop for OwnedPort {
    fn drop(&mut self) {
        if let Some(native) = self.native.take() {
            if native.live_handles() != 0 {
                // Keep the complete uncertain native fields and their outside credit together.
                std::mem::forget(native);
                if let Some(allocation) = self._allocation.take() {
                    std::mem::forget(allocation);
                }
            } else {
                drop(native);
            }
        }
    }
}
impl State {
    pub(super) fn stage_slots(&self) -> usize {
        self.stages.len()
            + if self.finished_owner.is_some() {
                finished_owner::FINISHED_METADATA_STAGES
            } else {
                0
            }
    }

    pub(super) fn live_handles(&self) -> usize {
        self.port.as_ref().map_or(self.retiring_handles, OwnedPort::live_handles)
            + self.capture_handles
            + if self.finished_owner.is_some() {
                finished_owner::FINISHED_NATIVE_OUTPUTS
            } else {
                0
            }
    }
    pub(super) fn retained_files(&self) -> usize {
        self.port.as_ref().map_or(self.retiring_files, OwnedPort::retained_files)
    }
}
