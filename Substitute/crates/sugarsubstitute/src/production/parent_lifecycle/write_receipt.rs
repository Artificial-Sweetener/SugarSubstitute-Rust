// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match inert actual-issue/completion metadata while the real write guard stays retained.
use super::ParentError;

pub(super) struct WriteReceipt {
    pub(super) operation: u64,
    bytes: usize,
    issued: bool,
}
impl WriteReceipt {
    pub(super) const fn new(operation: u64, bytes: usize) -> Self {
        Self { operation, bytes, issued: false }
    }
    pub(super) fn issue(&mut self, operation: u64) -> Result<(), ParentError> {
        if self.operation != operation || self.issued {
            return Err(ParentError::WrongPhase);
        }
        self.issued = true;
        Ok(())
    }
    pub(super) fn complete(&self, operation: u64, bytes: u32) -> Result<(), ParentError> {
        if self.operation != operation || !self.issued || self.bytes != bytes as usize {
            return Err(ParentError::WrongPhase);
        }
        Ok(())
    }
    pub(super) fn revoked(&self, operation: u64) -> Result<(), ParentError> {
        if self.operation != operation || self.issued {
            return Err(ParentError::WrongPhase);
        }
        Ok(())
    }
}
