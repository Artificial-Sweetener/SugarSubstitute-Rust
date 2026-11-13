// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate and extract a flat registry-layout pack archive.
//!
//! A registry archive is a zip whose members sit at the pack root, as `comfy node pack`
//! produces and the Comfy Registry serves. The member listing, in archive order, becomes the
//! `.tracking` record. The zip reader is not trusted with policy: every name is validated
//! here before anything is written, only plain files and directories are accepted, and sizes
//! are bounded both as declared and as actually produced while reading.

use super::digest_record::FileDigests;
use super::entry::{EntryName, parse_entry, validate_entry_set};
use super::error::{ErrorCode, PackInstallError, Result};
use crate::durable_storage::pack_tree::PackTree;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read, Seek, Write};

const FILE_TYPE_MASK: u32 = 0o170_000;
const REGULAR_FILE: u32 = 0o100_000;
const DIRECTORY: u32 = 0o040_000;
const READ_CHUNK_BYTES: usize = 1024 * 1024;

/// Bounds one archive must satisfy before and while it is extracted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveLimits {
    /// The most members.
    pub max_entries: usize,
    /// The most bytes all files may expand to.
    pub max_total_bytes: u64,
    /// The most bytes one file may expand to.
    pub max_file_bytes: u64,
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            max_entries: 20_000,
            max_total_bytes: 256 * 1024 * 1024,
            max_file_bytes: 64 * 1024 * 1024,
        }
    }
}

/// A validated extraction: its entries in archive order and each file's digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractedArchive {
    /// The member entries, which become `.tracking`.
    pub entries: Vec<EntryName>,
    /// Each extracted file's SHA-256.
    pub file_digests: FileDigests,
}

type Archive<R> = zip::ZipArchive<R>;

fn open<R: Read + Seek>(source: R) -> Result<Archive<R>> {
    zip::ZipArchive::new(source).map_err(|_| not_a_zip())
}

fn not_a_zip() -> PackInstallError {
    PackInstallError::format(ErrorCode::NotAZip, "The download is not a readable zip.")
}

/// Return the validated member entries of an archive without extracting it.
///
/// # Errors
/// Returns the shared refusal for a corrupt container, an unsafe, reserved or duplicate
/// name, a link, device or encrypted member, or a declared size or count over its bound.
pub fn list_registry_archive(bytes: &[u8], limits: ArchiveLimits) -> Result<Vec<EntryName>> {
    let mut archive = open(Cursor::new(bytes))?;
    validated_members(&mut archive, limits)
        .map(|members| members.into_iter().map(|(_, entry)| entry).collect())
}

/// Extract a validated archive held in memory into `target`, an empty folder the caller owns.
#[cfg(test)]
pub(crate) fn extract_registry_archive(
    bytes: &[u8],
    target: &PackTree,
    limits: ArchiveLimits,
) -> Result<ExtractedArchive> {
    extract_registry_source(Cursor::new(bytes), target, limits)
}

/// Extract a validated archive read from `source` (a file or memory) into `target`, an empty
/// folder the caller owns. Each member streams to its file in bounded chunks, so memory use
/// does not grow with the archive.
pub(crate) fn extract_registry_source<R: Read + Seek>(
    source: R,
    target: &PackTree,
    limits: ArchiveLimits,
) -> Result<ExtractedArchive> {
    let mut archive = open(source)?;
    let members = validated_members(&mut archive, limits)?;
    target.create()?;
    let mut total = 0_u64;
    let mut file_digests = FileDigests::new();
    for (index, entry) in &members {
        if entry.is_directory() {
            target.create_directories(entry.path())?;
            continue;
        }
        if let Some((parent, _)) = entry.path().rsplit_once('/') {
            target.create_directories(parent)?;
        }
        let mut digest = None;
        let mut refusal = None;
        let written = target.replace_with(entry.path(), |output| {
            match copy_member(&mut archive, *index, output, &mut total, limits) {
                Ok(value) => {
                    digest = Some(value);
                    Ok(())
                }
                Err(cause) => {
                    refusal = Some(cause);
                    Err(std::io::Error::other("The archive member was refused"))
                }
            }
        });
        if let Some(cause) = refusal {
            return Err(cause);
        }
        written?;
        let digest = digest.ok_or_else(not_a_zip)?;
        file_digests.insert(entry.path().to_owned(), digest);
    }
    let entries = members.into_iter().map(|(_, entry)| entry).collect();
    Ok(ExtractedArchive { entries, file_digests })
}

/// Return each file's SHA-256 as `source` would extract, validating it exactly as extraction
/// does but writing nothing.
pub(crate) fn digest_registry_source<R: Read + Seek>(
    source: R,
    limits: ArchiveLimits,
) -> Result<FileDigests> {
    let mut archive = open(source)?;
    let members = validated_members(&mut archive, limits)?;
    let mut total = 0_u64;
    let mut file_digests = FileDigests::new();
    for (index, entry) in members.iter().filter(|(_, entry)| !entry.is_directory()) {
        let digest = copy_member(&mut archive, *index, &mut std::io::sink(), &mut total, limits)?;
        file_digests.insert(entry.path().to_owned(), digest);
    }
    Ok(file_digests)
}

fn validated_members<R: Read + Seek>(
    archive: &mut Archive<R>,
    limits: ArchiveLimits,
) -> Result<Vec<(usize, EntryName)>> {
    let mut named = Vec::new();
    for index in 0..archive.len() {
        if !archive.by_index_raw(index).map_err(|_| not_a_zip())?.name().is_empty() {
            named.push(index);
        }
    }
    if named.is_empty() {
        return Err(empty());
    }
    if named.len() > limits.max_entries {
        return Err(PackInstallError::format(
            ErrorCode::TooManyEntries,
            "The archive has too many members.",
        ));
    }
    let mut members = Vec::new();
    let mut declared_total = 0_u64;
    for index in named {
        let member = archive.by_index_raw(index).map_err(|_| not_a_zip())?;
        let entry = parse_entry(member.name())?;
        if member.encrypted() {
            return Err(PackInstallError::format(
                ErrorCode::EncryptedMember,
                "The archive has encrypted members.",
            ));
        }
        let kind = member.unix_mode().map_or(0, |mode| mode & FILE_TYPE_MASK);
        if !matches!(kind, 0 | REGULAR_FILE | DIRECTORY)
            || entry.is_directory() != (member.is_dir() || kind == DIRECTORY)
        {
            return Err(PackInstallError::format(
                ErrorCode::LinkMember,
                "The archive has link or device members.",
            ));
        }
        declared_total = declared_total.saturating_add(member.size());
        if member.size() > limits.max_file_bytes || declared_total > limits.max_total_bytes {
            return Err(too_large());
        }
        members.push((index, entry));
    }
    let entries: Vec<EntryName> = members.iter().map(|(_, entry)| entry.clone()).collect();
    validate_entry_set(&entries)?;
    if entries.iter().all(EntryName::is_directory) {
        return Err(empty());
    }
    Ok(members)
}

/// Stream one member into `output`, enforcing the produced-size bounds; returns its SHA-256.
fn copy_member<R: Read + Seek>(
    archive: &mut Archive<R>,
    index: usize,
    output: &mut impl Write,
    total: &mut u64,
    limits: ArchiveLimits,
) -> Result<String> {
    let mut member = archive.by_index(index).map_err(|_| not_a_zip())?;
    let mut digest = Sha256::new();
    let mut file_bytes = 0_u64;
    let mut buffer = vec![0_u8; READ_CHUNK_BYTES];
    loop {
        let read = member.read(&mut buffer).map_err(|_| not_a_zip())?;
        if read == 0 {
            break;
        }
        let produced = u64::try_from(read).map_err(|_| too_large())?;
        *total = total.saturating_add(produced);
        file_bytes = file_bytes.saturating_add(produced);
        if file_bytes > limits.max_file_bytes || *total > limits.max_total_bytes {
            return Err(too_large());
        }
        digest.update(&buffer[..read]);
        output.write_all(&buffer[..read])?;
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn empty() -> PackInstallError {
    PackInstallError::format(ErrorCode::EmptyArchive, "The archive holds no files.")
}

fn too_large() -> PackInstallError {
    PackInstallError::format(ErrorCode::TooLarge, "The archive expands beyond the limit.")
}
