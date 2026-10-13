// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Write canonical record bytes into one admitted fixed capacity destination.
use super::*;
use std::io::{self, Write};

/// Own a bounded record buffer and its original residual allocation loan.
/// Bytes drop before the loan; borrowing bytes grants no filesystem authority.
pub struct DiagnosticRecordBuffer {
    pub(super) bytes: Vec<u8>,
    pub(super) _allocation: ledger::Residual,
}
impl DiagnosticRecordBuffer {
    /// Borrow actual serialized bytes without extracting the allocation.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}
impl Write for DiagnosticRecordBuffer {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if self.bytes.len().checked_add(input.len()).is_none_or(|n| n > DIAGNOSTIC_IO_BYTES) {
            return Err(io::Error::from(io::ErrorKind::OutOfMemory));
        }
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
