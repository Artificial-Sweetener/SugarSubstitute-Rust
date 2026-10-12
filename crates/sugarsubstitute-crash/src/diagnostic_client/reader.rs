// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read only exact bounded little-endian fields without allocations.
use super::DiagnosticWireError as E;
pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    pub(super) fn take(&mut self, n: usize) -> Result<&'a [u8], E> {
        let until = self.at.checked_add(n).ok_or(E::Capacity)?;
        let bytes = self.bytes.get(self.at..until).ok_or(E::Shape)?;
        self.at = until;
        Ok(bytes)
    }
    pub(super) fn array<const N: usize>(&mut self) -> Result<[u8; N], E> {
        self.take(N)?.try_into().map_err(|_| E::Shape)
    }
    pub(super) fn u8(&mut self) -> Result<u8, E> {
        Ok(self.array::<1>()?[0])
    }
    pub(super) fn u16(&mut self) -> Result<u16, E> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    pub(super) fn u32(&mut self) -> Result<u32, E> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    pub(super) fn u64(&mut self) -> Result<u64, E> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    pub(super) fn i64(&mut self) -> Result<i64, E> {
        Ok(i64::from_le_bytes(self.array()?))
    }
    pub(super) fn boolean(&mut self) -> Result<bool, E> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(E::Schema),
        }
    }
    pub(super) fn finish(self) -> Result<(), E> {
        if self.at == self.bytes.len() { Ok(()) } else { Err(E::Shape) }
    }
}
