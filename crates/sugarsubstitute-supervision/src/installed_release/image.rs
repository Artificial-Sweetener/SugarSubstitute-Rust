// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read bounded PE/PDB matching identities from one retained source without reopening labels.
#[path = "image/pdb.rs"]
mod pdb;
#[path = "image/pe.rs"]
mod pe;
use std::{error::Error, fmt};

/// Borrow the selected native file through its existing checked bounded reader.
pub(super) trait ReadAt {
    /// Preserve the original native/source failure without flattening it.
    type Error: Error + Send + Sync + 'static;
    /// Return the already whole-hash-validated source length in bytes.
    fn length(&self) -> u64;
    /// Read the exact byte range from the same retained file and original context.
    fn read_at(&mut self, offset: u64, output: &mut [u8]) -> Result<(), Self::Error>;
}
/// Reject malformed matching data while preserving an external read cause.
#[derive(Debug)]
pub(super) enum ImageError<E> {
    /// A bounded parser or native/source range is invalid.
    Invalid(&'static str),
    /// The original source failed or its context stopped.
    Read(E),
}
impl<E: fmt::Display> fmt::Display for ImageError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(reason) => write!(f, "Release image identity rejected at {reason}"),
            Self::Read(_) => f.write_str("Release image identity read failed"),
        }
    }
}
impl<E: Error + 'static> Error for ImageError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Invalid(_) => None,
        }
    }
}
/// Match the actual RSDS and PDB information stream identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PdbIdentity {
    pub(super) guid: [u8; 16],
    pub(super) age: u32,
}
/// Distinguish actual executable and DLL flags, independently of a path extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PeKind {
    Executable,
    Library,
}
/// Keep only bounded actual image facts needed by the signed manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PeIdentity {
    pub(super) kind: PeKind,
    pub(super) image_size: u32,
    pub(super) timestamp: u32,
    pub(super) checksum: u32,
    pub(super) pdb: PdbIdentity,
}
fn bytes<const N: usize, R: ReadAt>(
    source: &mut R,
    offset: u64,
) -> Result<[u8; N], ImageError<R::Error>> {
    if offset.checked_add(N as u64).is_none_or(|end| end > source.length()) {
        return Err(ImageError::Invalid("file_range"));
    }
    let mut output = [0; N];
    source.read_at(offset, &mut output).map_err(ImageError::Read)?;
    Ok(output)
}
fn number<const N: usize, E>(input: &[u8], offset: usize) -> Result<[u8; N], ImageError<E>> {
    input
        .get(offset..offset + N)
        .and_then(|slice| slice.try_into().ok())
        .ok_or(ImageError::Invalid("field_range"))
}
fn u16_at<E>(input: &[u8], offset: usize) -> Result<u16, ImageError<E>> {
    Ok(u16::from_le_bytes(number(input, offset)?))
}
fn u32_at<E>(input: &[u8], offset: usize) -> Result<u32, ImageError<E>> {
    Ok(u32::from_le_bytes(number(input, offset)?))
}
pub(super) use pdb::read_pdb;
pub(super) use pe::read_pe;
#[cfg(test)]
#[path = "image/tests.rs"]
mod tests;
