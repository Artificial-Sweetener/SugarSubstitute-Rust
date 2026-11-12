// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate exact UTF-16LE views and UTF-8 budgets before explicit owned conversion.
use super::{ProtocolError, binary::Reader};
use std::fmt;
use sugarsubstitute_contracts::configuration::{
    MAX_ARGUMENT_BYTES, MAX_ARGUMENTS, MAX_TOTAL_ARGUMENT_BYTES,
};

/// Borrow strict, non-NUL UTF-16LE without normalizing its Unicode spelling.
#[derive(Clone, Copy)]
pub struct Utf16Text<'a> {
    bytes: &'a [u8],
    utf8_bytes: usize,
}
impl<'a> Utf16Text<'a> {
    pub(super) fn read(
        reader: &mut Reader<'a>,
        maximum_units: usize,
    ) -> Result<Self, ProtocolError> {
        let units = usize::try_from(reader.u32()?).map_err(|_| ProtocolError::Length)?;
        if units > maximum_units {
            return Err(ProtocolError::Length);
        }
        Self::validate(reader.take(units.checked_mul(2).ok_or(ProtocolError::Length)?)?)
    }
    pub(super) fn validate(bytes: &'a [u8]) -> Result<Self, ProtocolError> {
        if !bytes.len().is_multiple_of(2) {
            return Err(ProtocolError::InvalidUtf16);
        }
        let mut utf8_bytes = 0usize;
        for character in char::decode_utf16(units(bytes)) {
            let character = character.map_err(|_| ProtocolError::InvalidUtf16)?;
            if character == '\0' {
                return Err(ProtocolError::InteriorNul);
            }
            utf8_bytes =
                utf8_bytes.checked_add(character.len_utf8()).ok_or(ProtocolError::Length)?;
        }
        Ok(Self { bytes, utf8_bytes })
    }
    /// Number of exact native UTF-16 units, excluding any terminator.
    pub const fn unit_len(self) -> usize {
        self.bytes.len() / 2
    }
    /// Exact eventual UTF-8 byte count, measured during allocation-free preflight.
    pub const fn utf8_len(self) -> usize {
        self.utf8_bytes
    }
    /// Iterate exact original code units without alignment assumptions or copying.
    pub fn units(self) -> impl Iterator<Item = u16> + 'a {
        units(self.bytes)
    }
    /// Allocate the supported Unicode spelling only when the admitted caller asks.
    /// Allocation errors retain their cause; unmatched units are never substituted.
    pub fn to_owned_string(self) -> Result<String, ProtocolError> {
        let mut output = String::new();
        output.try_reserve_exact(self.utf8_bytes).map_err(ProtocolError::Allocation)?;
        for character in char::decode_utf16(self.units()) {
            output.push(character.map_err(|_| ProtocolError::InvalidUtf16)?);
        }
        Ok(output)
    }
}
impl fmt::Debug for Utf16Text<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Utf16Text").field("units", &self.unit_len()).finish_non_exhaustive()
    }
}
fn units(bytes: &[u8]) -> impl Iterator<Item = u16> + '_ {
    bytes.chunks_exact(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
}

/// Borrow a fully preflighted length-prefixed argv list without allocating a Vec.
#[derive(Clone, Copy)]
pub struct ArgumentRefs<'a> {
    bytes: &'a [u8],
    count: usize,
}
impl fmt::Debug for ArgumentRefs<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArgumentRefs").field("count", &self.count).finish_non_exhaustive()
    }
}
impl<'a> ArgumentRefs<'a> {
    pub(super) fn read(
        reader: &mut Reader<'a>,
        count: usize,
        source: &'a [u8],
    ) -> Result<Self, ProtocolError> {
        if count > MAX_ARGUMENTS {
            return Err(ProtocolError::Length);
        }
        let start = reader.position();
        let mut total = 0usize;
        for _ in 0..count {
            let argument = Utf16Text::read(reader, MAX_ARGUMENT_BYTES)?;
            if argument.utf8_len() > MAX_ARGUMENT_BYTES {
                return Err(ProtocolError::Length);
            }
            total = total.checked_add(argument.utf8_len()).ok_or(ProtocolError::Length)?;
            if total > MAX_TOTAL_ARGUMENT_BYTES {
                return Err(ProtocolError::Length);
            }
        }
        let bytes = source.get(start..reader.position()).ok_or(ProtocolError::Truncated)?;
        Ok(Self { bytes, count })
    }
    /// Exact argument count; empty arguments still count and retain their position.
    pub const fn len(self) -> usize {
        self.count
    }
    /// Whether there are no arguments following argv0.
    pub const fn is_empty(self) -> bool {
        self.count == 0
    }
    /// Iterate validated argument views, preserving order and empty values.
    pub fn iter(self) -> impl Iterator<Item = Result<Utf16Text<'a>, ProtocolError>> {
        let mut reader = Reader::new(self.bytes);
        (0..self.count).map(move |_| Utf16Text::read(&mut reader, MAX_ARGUMENT_BYTES))
    }
    pub(super) fn owned(self) -> Result<Vec<String>, ProtocolError> {
        let mut output = Vec::new();
        output.try_reserve_exact(self.count).map_err(ProtocolError::Allocation)?;
        for argument in self.iter() {
            output.push(argument?.to_owned_string()?);
        }
        Ok(output)
    }
}
