// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep real native issuance and complete byte counts separate from queue admission.
use super::*;
/// Created only after actual native queue admission; it never proves peer processing.
pub(super) struct Receipt {
    pub(super) operation: LifecyclePipeOperationId,
    pub(super) sequence: u64,
    pub(super) bytes: usize,
    issued: bool,
    returned: bool,
}
impl Receipt {
    pub(super) fn new(operation: LifecyclePipeOperationId, sequence: u64, bytes: usize) -> Self {
        Self { operation, sequence, bytes, issued: false, returned: false }
    }
    pub(super) fn issued(&mut self, operation: LifecyclePipeOperationId) -> Result<()> {
        if self.operation != operation || self.issued || self.returned {
            return Err("Recovery native write issue rejected".into());
        }
        self.issued = true;
        Ok(())
    }
    pub(super) fn written(
        &self,
        operation: LifecyclePipeOperationId,
        bytes: u32,
        previous: u64,
    ) -> Result<()> {
        if self.operation != operation
            || !self.issued
            || self.returned
            || bytes as usize != self.bytes
            || previous.checked_add(1) != Some(self.sequence)
        {
            return Err("Recovery native write completion rejected".into());
        }
        Ok(())
    }
    pub(super) fn returned(&mut self, operation: LifecyclePipeOperationId) -> Result<()> {
        if self.operation != operation || self.issued || self.returned {
            return Err("Recovery unissued native write return rejected".into());
        }
        self.returned = true;
        Ok(())
    }
}
#[cfg(test)]
#[path = "receipt_tests.rs"]
mod tests;
