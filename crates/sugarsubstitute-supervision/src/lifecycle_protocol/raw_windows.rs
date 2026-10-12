// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow canonical Windows folder fragments without Unicode conversion or allocation.
use super::{
    ProtocolError,
    binary::{Reader, Writer},
    main_fields,
};
use std::fmt;
use sugarsubstitute_contracts::{lifecycle::RunKey, startup::MainIdentity};

/// Largest native profile folder, excluding its terminal NUL, in UTF16 code units.
pub const MAX_PROFILE_FOLDER_UNITS: usize = 32_766;
/// Fixed canonical fragment partition, in UTF16 code units.
pub const MAX_FOLDER_FRAGMENT_UNITS: usize = 16_384;

#[derive(Clone, Copy)]
enum Storage<'a> {
    Units(&'a [u16]),
    Bytes(&'a [u8]),
}
/// Borrow exact non-NUL Windows units, including unmatched surrogates.
/// This validates one bounded fragment's shape, never a filesystem identity or authority.
#[derive(Clone, Copy)]
pub struct RawWindowsUnits<'a>(Storage<'a>);
impl<'a> RawWindowsUnits<'a> {
    /// Borrow 1..16384 units without copying or interpreting Unicode.
    /// # Errors
    /// Reject empty/oversized input or any NUL unit.
    pub fn from_units(units: &'a [u16]) -> Result<Self, ProtocolError> {
        Self::validate(Self(Storage::Units(units)))
    }
    /// Borrow exact unaligned UTF16LE bytes without allocating or replacing surrogates.
    /// # Errors
    /// Reject odd byte counts, empty/oversized input or any NUL unit.
    pub fn from_le_bytes(bytes: &'a [u8]) -> Result<Self, ProtocolError> {
        if !bytes.len().is_multiple_of(2) {
            return Err(ProtocolError::RawWindowsPath);
        }
        Self::validate(Self(Storage::Bytes(bytes)))
    }
    fn validate(value: Self) -> Result<Self, ProtocolError> {
        if !(1..=MAX_FOLDER_FRAGMENT_UNITS).contains(&value.unit_len()) {
            return Err(ProtocolError::RawWindowsPath);
        }
        if value.units().any(|unit| unit == 0) {
            return Err(ProtocolError::InteriorNul);
        }
        Ok(value)
    }
    /// Return exact unit count excluding any terminator.
    pub fn unit_len(self) -> usize {
        match self.0 {
            Storage::Units(units) => units.len(),
            Storage::Bytes(bytes) => bytes.len() / 2,
        }
    }
    /// Iterate original units with no alignment assumption or copy.
    pub fn units(self) -> impl Iterator<Item = u16> + 'a {
        (0..self.unit_len()).filter_map(move |index| match self.0 {
            Storage::Units(units) => units.get(index).copied(),
            Storage::Bytes(bytes) => bytes
                .get(index * 2..index * 2 + 2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]])),
        })
    }
}
impl PartialEq for RawWindowsUnits<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.unit_len() == other.unit_len() && self.units().eq(other.units())
    }
}
impl fmt::Debug for RawWindowsUnits<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawWindowsUnits").field("units", &self.unit_len()).finish_non_exhaustive()
    }
}

/// One of at most two canonical fragments of an exact-current profile folder.
/// Assembly ordering, duplicate history and authentication remain with the composing owner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FolderFragment<'a> {
    identity: MainIdentity,
    total_units: u32,
    offset_units: u32,
    units: RawWindowsUnits<'a>,
}
impl<'a> FolderFragment<'a> {
    /// Validate total1..32766, offset0 or16384 and exact canonical chunk count.
    /// # Errors
    /// Reject a third, partial, overlapping or noncanonical partition.
    pub fn new(
        identity: MainIdentity,
        total_units: u32,
        offset_units: u32,
        units: RawWindowsUnits<'a>,
    ) -> Result<Self, ProtocolError> {
        let total = usize::try_from(total_units).map_err(|_| ProtocolError::FragmentShape)?;
        let offset = usize::try_from(offset_units).map_err(|_| ProtocolError::FragmentShape)?;
        if !(1..=MAX_PROFILE_FOLDER_UNITS).contains(&total)
            || ![0, MAX_FOLDER_FRAGMENT_UNITS].contains(&offset)
            || offset >= total
            || units.unit_len() != (total - offset).min(MAX_FOLDER_FRAGMENT_UNITS)
        {
            return Err(ProtocolError::FragmentShape);
        }
        Ok(Self { identity, total_units, offset_units, units })
    }
    /// Return the complete claimed main identity, granting no native authority.
    pub fn identity(self) -> MainIdentity {
        self.identity
    }
    /// Return original complete folder size in code units.
    pub fn total_units(self) -> u32 {
        self.total_units
    }
    /// Return exact original canonical offset in code units.
    pub fn offset_units(self) -> u32 {
        self.offset_units
    }
    /// Borrow exact bounded raw units, retaining their caller/frame lifetime.
    pub fn units(self) -> RawWindowsUnits<'a> {
        self.units
    }
    pub(super) fn read(reader: &mut Reader<'a>, run: RunKey) -> Result<Self, ProtocolError> {
        let identity = main_fields::read_identity(reader, run)?;
        let total = reader.u32()?;
        let offset = reader.u32()?;
        let count = usize::try_from(reader.u32()?).map_err(|_| ProtocolError::FragmentShape)?;
        if !(1..=MAX_FOLDER_FRAGMENT_UNITS).contains(&count) {
            return Err(ProtocolError::FragmentShape);
        }
        let units = RawWindowsUnits::from_le_bytes(reader.take(count * 2)?)?;
        Self::new(identity, total, offset, units)
    }
    pub(super) fn write(self, run: RunKey, writer: &mut Writer<'_>) -> Result<(), ProtocolError> {
        main_fields::write_identity(self.identity, run, writer)?;
        writer.u32(self.total_units)?;
        writer.u32(self.offset_units)?;
        writer.u32(self.units.unit_len() as u32)?;
        for unit in self.units.units() {
            writer.u16(unit)?;
        }
        Ok(())
    }
}
