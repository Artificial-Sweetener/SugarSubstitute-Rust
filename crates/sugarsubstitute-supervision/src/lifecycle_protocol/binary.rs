// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read and preflight/write bounded little-endian fields without native casts or allocation.
use super::{MAX_FRAME_BYTES, ProtocolError};

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

pub(super) struct Writer<'a> {
    pub(super) output: Option<&'a mut [u8]>,
    pub(super) position: usize,
}
impl Writer<'_> {
    pub(super) fn measure() -> Self {
        Self { output: None, position: 0 }
    }
    pub(super) fn put(&mut self, bytes: &[u8]) -> Result<(), ProtocolError> {
        let end = self.position.checked_add(bytes.len()).ok_or(ProtocolError::Length)?;
        if end > MAX_FRAME_BYTES {
            return Err(ProtocolError::FrameTooLarge);
        }
        if let Some(output) = &mut self.output {
            let destination = output.get_mut(self.position..end).ok_or(ProtocolError::Length)?;
            destination.copy_from_slice(bytes);
        }
        self.position = end;
        Ok(())
    }
    pub(super) fn u8(&mut self, value: u8) -> Result<(), ProtocolError> {
        self.put(&[value])
    }
    pub(super) fn u16(&mut self, value: u16) -> Result<(), ProtocolError> {
        self.put(&value.to_le_bytes())
    }
    pub(super) fn u32(&mut self, value: u32) -> Result<(), ProtocolError> {
        self.put(&value.to_le_bytes())
    }
    pub(super) fn u64(&mut self, value: u64) -> Result<(), ProtocolError> {
        self.put(&value.to_le_bytes())
    }
    pub(super) fn text(&mut self, value: &str, maximum_units: usize) -> Result<(), ProtocolError> {
        let units = value.encode_utf16().count();
        if units > maximum_units {
            return Err(ProtocolError::Length);
        }
        if value.contains('\0') {
            return Err(ProtocolError::InteriorNul);
        }
        self.u32(u32::try_from(units).map_err(|_| ProtocolError::Length)?)?;
        for unit in value.encode_utf16() {
            self.u16(unit)?;
        }
        Ok(())
    }
}
