// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read checked unaligned little-endian fields without native casts or allocation.
use super::ProtocolError;

pub(super) struct Reader<'a> {
    source: &'a [u8],
    position: usize,
}
impl<'a> Reader<'a> {
    pub(super) fn new(source: &'a [u8]) -> Self {
        Self { source, position: 0 }
    }
    pub(super) fn position(&self) -> usize {
        self.position
    }
    pub(super) fn take(&mut self, bytes: usize) -> Result<&'a [u8], ProtocolError> {
        let end = self.position.checked_add(bytes).ok_or(ProtocolError::Length)?;
        let result = self.source.get(self.position..end).ok_or(ProtocolError::Truncated)?;
        self.position = end;
        Ok(result)
    }
    pub(super) fn array<const N: usize>(&mut self) -> Result<&'a [u8; N], ProtocolError> {
        self.take(N)?.try_into().map_err(|_| ProtocolError::Truncated)
    }
    pub(super) fn u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.array::<1>()?[0])
    }
    pub(super) fn u16(&mut self) -> Result<u16, ProtocolError> {
        Ok(u16::from_le_bytes(*self.array()?))
    }
    pub(super) fn u32(&mut self) -> Result<u32, ProtocolError> {
        Ok(u32::from_le_bytes(*self.array()?))
    }
    pub(super) fn u64(&mut self) -> Result<u64, ProtocolError> {
        Ok(u64::from_le_bytes(*self.array()?))
    }
    pub(super) fn i64(&mut self) -> Result<i64, ProtocolError> {
        Ok(i64::from_le_bytes(*self.array()?))
    }
    pub(super) fn finish(self) -> Result<(), ProtocolError> {
        if self.position == self.source.len() { Ok(()) } else { Err(ProtocolError::TrailingBytes) }
    }
}
