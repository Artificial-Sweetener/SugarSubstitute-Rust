// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode bounded matching release facts without creating file or signer authority.
//!
//! The installed-release owner authenticates the original bytes before using these
//! labels to select native files. This parser performs no filesystem or key lookup.

use std::str;

const MAGIC: &[u8; 16] = b"SSRELEASE-K1-v1\0";
const CAPTURE_MAGIC: &[u8; 16] = b"SSRELEASE-K1-v2\0";
pub(super) const MAX_MANIFEST_BYTES: usize = 65_536;
const MAX_PAIRS: usize = 16;

/// Reject malformed or unknown matching facts before native selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ManifestError {
    /// Input or a declared field exceeds its immutable bound.
    Bound,
    /// A declared field extends beyond the retained input.
    Truncated,
    /// The wire version, development identity or architecture is unsupported.
    UnknownSchema,
    /// A role, leaf, number or matching identity is malformed.
    InvalidField,
    /// Repeated or unordered leaves make selection ambiguous.
    Ambiguous,
    /// Host and helper do not select exactly one common executable.
    RoleMismatch,
    /// The authenticated source generation differs from the compiled local build.
    #[cfg(feature = "local-k1-symbol-trust")]
    BuildMismatch,
    /// Bytes remain after the declared bounded records.
    Trailing,
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Authenticated release manifest rejected: {self:?}")
    }
}
impl std::error::Error for ManifestError {}

/// Borrow one bounded manifest pair; every field is a matching fact.
#[derive(Clone, Copy, Debug)]
pub(super) struct Pair<'a> {
    pub(super) roles: u8,
    pub(super) pe_label: &'a str,
    pub(super) pdb_label: &'a str,
    pub(super) pe_length: u64,
    pub(super) pe_sha256: [u8; 32],
    pub(super) pdb_length: u64,
    pub(super) pdb_sha256: [u8; 32],
    pub(super) image_size: u32,
    pub(super) timestamp: u32,
    pub(super) checksum: u32,
    pub(super) pdb_guid: [u8; 16],
    pub(super) pdb_age: u32,
}

/// Keep a fixed inventory borrowed from the original owned ingress buffer.
#[derive(Debug)]
pub(super) struct Manifest<'a> {
    pub(super) version: u32,
    pub(super) key_id: [u8; 32],
    pub(super) build: &'a str,
    pairs: [Option<Pair<'a>>; MAX_PAIRS],
    #[cfg(test)]
    count: usize,
    role_index: usize,
    client_index: Option<usize>,
    handler_index: Option<usize>,
}
impl<'a> Manifest<'a> {
    /// Decode exact development schema1/2 facts without granting trust.
    ///
    /// # Errors
    /// Rejects oversized, truncated, unknown, ambiguous and trailing input.
    pub(super) fn parse(bytes: &'a [u8]) -> Result<Self, ManifestError> {
        if bytes.is_empty() || bytes.len() > MAX_MANIFEST_BYTES {
            return Err(ManifestError::Bound);
        }
        let mut input = Input { bytes, offset: 0 };
        let magic = input.take(16)?;
        let version = input.u32()?;
        if !((magic == MAGIC && version == 1) || (magic == CAPTURE_MAGIC && version == 2))
            || input.byte()? != 1
        {
            return Err(ManifestError::UnknownSchema);
        }
        let key_id = input.array()?;
        let build = input.text(64)?;
        if build.is_empty()
            || !build.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
        {
            return Err(ManifestError::InvalidField);
        }
        if input.u16()? != 0x8664 {
            return Err(ManifestError::UnknownSchema);
        }
        let count = usize::from(input.byte()?);
        if !(1..=MAX_PAIRS).contains(&count) {
            return Err(ManifestError::Bound);
        }
        let mut pairs: [Option<Pair<'a>>; MAX_PAIRS] = [None; MAX_PAIRS];
        let mut host = None;
        let mut helper = None;
        let mut client = None;
        let mut handler = None;
        for index in 0..count {
            let pair = Pair::read(&mut input, version)?;
            for prior in pairs[..index].iter().flatten() {
                if prior.pe_label >= pair.pe_label
                    || [prior.pe_label, prior.pdb_label].contains(&pair.pe_label)
                    || [prior.pe_label, prior.pdb_label].contains(&pair.pdb_label)
                {
                    return Err(ManifestError::Ambiguous);
                }
            }
            if pair.roles & 1 != 0 && host.replace(index).is_some() {
                return Err(ManifestError::RoleMismatch);
            }
            if pair.roles & 2 != 0 && helper.replace(index).is_some() {
                return Err(ManifestError::RoleMismatch);
            }
            if pair.roles & 8 != 0 && client.replace(index).is_some() {
                return Err(ManifestError::RoleMismatch);
            }
            if pair.roles & 16 != 0 && handler.replace(index).is_some() {
                return Err(ManifestError::RoleMismatch);
            }
            pairs[index] = Some(pair);
        }
        let role_index = host.ok_or(ManifestError::RoleMismatch)?;
        if helper != Some(role_index) {
            return Err(ManifestError::RoleMismatch);
        }
        if input.offset != bytes.len() {
            return Err(ManifestError::Trailing);
        }
        Ok(Self {
            version,
            key_id,
            build,
            pairs,
            #[cfg(test)]
            count,
            role_index,
            client_index: client,
            handler_index: handler,
        })
    }

    /// Match the authenticated generation against the compiled source-inventory literal.
    /// # Errors
    /// Rejects a differently signed build before its labels may select native files.
    #[cfg(feature = "local-k1-symbol-trust")]
    pub(super) fn require_compiled_build(&self) -> Result<(), ManifestError> {
        if self.build != super::local_k1_build::BUILD {
            return Err(ManifestError::BuildMismatch);
        }
        Ok(())
    }

    /// Borrow the exact common executable record; parsing established uniqueness.
    pub(super) fn role_pair(&self) -> Option<&Pair<'a>> {
        self.pairs[self.role_index].as_ref()
    }

    /// Borrow one exact named record; no caller-supplied path is resolved.
    #[cfg(test)]
    pub(super) fn find(&self, label: &str) -> Option<&Pair<'a>> {
        self.pairs[..self.count].iter().flatten().find(|pair| pair.pe_label == label)
    }
    /// Borrow the distinct fixed capture records from schema2 matching data.
    /// # Errors
    /// Schema1 never supplies capture association; both exact schema2 roles are required.
    pub(super) fn capture_pairs(&self) -> Result<(&Pair<'a>, &Pair<'a>), ManifestError> {
        if self.version != 2 {
            return Err(ManifestError::UnknownSchema);
        }
        let client = self.client_index.and_then(|index| self.pairs[index].as_ref());
        let handler = self.handler_index.and_then(|index| self.pairs[index].as_ref());
        client.zip(handler).ok_or(ManifestError::RoleMismatch)
    }
}
impl<'a> Pair<'a> {
    fn read(input: &mut Input<'a>, version: u32) -> Result<Self, ManifestError> {
        let roles = input.byte()?;
        let pe_label = input.text(128)?;
        let pdb_label = input.text(128)?;
        validate_label(pe_label, &["exe", "dll"])?;
        validate_label(pdb_label, &["pdb"])?;
        let allowed = if version == 2 { 31 } else { 7 };
        if roles == 0 || roles & !allowed != 0 || (roles & 3 != 0 && !pe_label.ends_with(".exe")) {
            return Err(ManifestError::InvalidField);
        }
        if roles & 8 != 0
            && (roles & !12 != 0
                || pe_label != "sugarsubstitute_crashpad_client.dll"
                || pdb_label != "sugarsubstitute_crashpad_client.pdb")
        {
            return Err(ManifestError::RoleMismatch);
        }
        if roles & 16 != 0
            && (roles & !20 != 0
                || pe_label != "crashpad_handler.exe"
                || pdb_label != "crashpad_handler.pdb")
        {
            return Err(ManifestError::RoleMismatch);
        }
        let pair = Self {
            roles,
            pe_label,
            pdb_label,
            pe_length: input.u64()?,
            pe_sha256: input.array()?,
            pdb_length: input.u64()?,
            pdb_sha256: input.array()?,
            image_size: input.u32()?,
            timestamp: input.u32()?,
            checksum: input.u32()?,
            pdb_guid: input.array()?,
            pdb_age: input.u32()?,
        };
        if pair.pe_length == 0
            || pair.pdb_length == 0
            || pair.image_size == 0
            || pair.pdb_guid == [0; 16]
            || pair.pdb_age == 0
        {
            return Err(ManifestError::InvalidField);
        }
        Ok(pair)
    }
}

fn validate_label(label: &str, extensions: &[&str]) -> Result<(), ManifestError> {
    let (stem, extension) = label.split_once('.').ok_or(ManifestError::InvalidField)?;
    let reserved = ["con", "prn", "aux", "nul"].contains(&stem)
        || (stem.len() == 4
            && (stem.starts_with("com") || stem.starts_with("lpt"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    if stem.is_empty()
        || reserved
        || !stem
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
        || !extensions.contains(&extension)
    {
        return Err(ManifestError::InvalidField);
    }
    Ok(())
}

struct Input<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Input<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], ManifestError> {
        let end = self.offset.checked_add(count).ok_or(ManifestError::Bound)?;
        let output = self.bytes.get(self.offset..end).ok_or(ManifestError::Truncated)?;
        self.offset = end;
        Ok(output)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], ManifestError> {
        self.take(N)?.try_into().map_err(|_| ManifestError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, ManifestError> {
        Ok(self.array::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, ManifestError> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, ManifestError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, ManifestError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn text(&mut self, bound: usize) -> Result<&'a str, ManifestError> {
        let count = usize::from(self.byte()?);
        if count > bound {
            return Err(ManifestError::Bound);
        }
        let bytes = self.take(count)?;
        if !bytes.is_ascii() {
            return Err(ManifestError::InvalidField);
        }
        str::from_utf8(bytes).map_err(|_| ManifestError::InvalidField)
    }
}

#[cfg(test)]
#[path = "manifest/tests.rs"]
mod tests;
