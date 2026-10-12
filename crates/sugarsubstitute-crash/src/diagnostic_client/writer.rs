// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Write preflighted headers to fixed caller storage; never grow buffers.
use super::DiagnosticWireError as E;
pub(super) struct Writer<'a> {
    bytes: &'a mut [u8],
    at: usize,
}
impl<'a> Writer<'a> {
    pub(super) fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    pub(super) fn put(&mut self, bytes: &[u8]) -> Result<(), E> {
        let until = self.at.checked_add(bytes.len()).ok_or(E::Capacity)?;
        self.bytes.get_mut(self.at..until).ok_or(E::Capacity)?.copy_from_slice(bytes);
        self.at = until;
        Ok(())
    }
    pub(super) fn u8(&mut self, n: u8) -> Result<(), E> {
        self.put(&[n])
    }
    pub(super) fn u16(&mut self, n: u16) -> Result<(), E> {
        self.put(&n.to_le_bytes())
    }
    pub(super) fn u32(&mut self, n: u32) -> Result<(), E> {
        self.put(&n.to_le_bytes())
    }
    pub(super) fn u64(&mut self, n: u64) -> Result<(), E> {
        self.put(&n.to_le_bytes())
    }
    pub(super) fn i64(&mut self, n: i64) -> Result<(), E> {
        self.put(&n.to_le_bytes())
    }
    pub(super) const fn length(&self) -> usize {
        self.at
    }
}
