// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode the common native archive through installer trust or retained authenticated download custody.
use super::{
    VerifiedDownload, check,
    files::{self, Directory},
    invalid,
};
use flate2::bufread::GzDecoder;
use sha2::{Digest, Sha256};
use std::{
    io::{self, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_contracts::update::Component;
use sugarsubstitute_update::package::{MAX_MANIFEST_BYTES, Manifest};
const MAGIC: &[u8; 8] = b"SSSETUP1";
mod extract;
mod footer;

/// Resolve a development bundle or unpack the compiled installer's authenticated inventory.
/// Extraction runs on an admitted storage worker and never executes a payload.
/// # Errors
/// Rejects malformed archives, untrusted inventories, traversal and changed payload bytes.
pub fn materialize(
    source: &Path,
    workspace: &Path,
    expected: &str,
    context: &JobContext,
) -> io::Result<PathBuf> {
    check(context)?;
    if source.is_dir() {
        return Ok(source.to_owned());
    }
    let _memory = context.memory().claim(super::RESERVATION_BYTES).map_err(io::Error::other)?;
    let parent = Directory::open(
        source.parent().ok_or_else(|| invalid("Installer parent missing"))?,
        false,
    )?;
    let leaf = source
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid("Installer filename invalid"))?;
    let mut file = parent.read(leaf)?;
    let archive = inventory(&mut file, expected)?;
    unpack(&mut file, archive, workspace, expected, Component::Application, context)
}

/// Authenticate the inventory through the exact verified archive handle, without reopening a path.
/// Bare update archives use the same format as installer payloads, with no executable envelope.
pub(super) fn downloaded(
    source: &mut VerifiedDownload,
    workspace: &Path,
    component: Component,
    context: &JobContext,
) -> io::Result<(PathBuf, String)> {
    check(context)?;
    let _memory = context.memory().claim(super::RESERVATION_BYTES).map_err(io::Error::other)?;
    let archive = read_inventory(source)?;
    if archive.offset != 0 || archive.end != source.identity().length() {
        return Err(invalid("Update package has an unexpected executable envelope"));
    }
    let digest = format!("{:x}", Sha256::digest(&archive.manifest));
    let root = unpack(source, archive, workspace, &digest, component, context)?;
    Ok((root, digest))
}

/// Exact bounded sections; this structural description grants no authenticity on its own.
struct Archive {
    manifest: Vec<u8>,
    compressed: u64,
    offset: u64,
    end: u64,
}
/// Fixed section bounds shared by extraction and signed installer-package import.
struct Layout {
    compressed: u64,
    manifest_size: u64,
    offset: u64,
    end: u64,
}

/// Locate an embedded bare archive without trusting its inventory or allocating from it.
/// The caller must authenticate every byte in this interval before extracting anything.
pub(super) fn embedded_range(file: &mut (impl Read + Seek)) -> io::Result<std::ops::Range<u64>> {
    let layout = layout(file)?;
    if layout.offset == 0 {
        return Err(invalid("Installer executable envelope is absent"));
    }
    Ok(layout.offset..layout.end)
}
/// The trusted installer authenticates inventory separately from its executable envelope.
fn inventory(file: &mut (impl Read + Seek), expected: &str) -> io::Result<Archive> {
    let archive = read_inventory(file)?;
    if format!("{:x}", Sha256::digest(&archive.manifest)) != expected {
        return Err(invalid("Installer archive authentication failed"));
    }
    Ok(archive)
}
/// Parse fixed footer bounds before allocating or consulting untrusted inventory data.
fn read_inventory(file: &mut (impl Read + Seek)) -> io::Result<Archive> {
    let Layout { compressed, manifest_size, offset, end } = layout(file)?;
    file.seek(SeekFrom::Start(offset + compressed))?;
    let mut manifest = vec![0; manifest_size as usize];
    file.read_exact(&mut manifest)?;
    Ok(Archive { manifest, compressed, offset, end })
}

/// Interpret one bounded footer, including the existing optional PE certificate table.
fn layout(file: &mut (impl Read + Seek)) -> io::Result<Layout> {
    let end = footer::end(file)?;
    file.seek(SeekFrom::Start(end - 24))?;
    let mut footer = [0; 24];
    file.read_exact(&mut footer)?;
    if &footer[16..] != MAGIC {
        return Err(invalid("Installer payload footer is invalid"));
    }
    let compressed = u64::from_le_bytes(footer[..8].try_into().map_err(io::Error::other)?);
    let manifest_size = u64::from_le_bytes(footer[8..16].try_into().map_err(io::Error::other)?);
    if manifest_size > MAX_MANIFEST_BYTES as u64 || compressed > 16 * 1024 * 1024 * 1024 {
        return Err(invalid("Installer archive exceeds its bound"));
    }
    let offset = end
        .checked_sub(24 + manifest_size + compressed)
        .ok_or_else(|| invalid("Installer archive is truncated"))?;
    Ok(Layout { compressed, manifest_size, offset, end })
}
/// Both entry paths use identical validation, decompression and create-only extraction rules.
fn unpack(
    file: &mut (impl Read + Seek),
    archive: Archive,
    workspace: &Path,
    digest: &str,
    component: Component,
    context: &JobContext,
) -> io::Result<PathBuf> {
    let package = Manifest::decode(&archive.manifest).map_err(io::Error::other)?;
    super::inventory::require_component(&package, component)?;
    check(context)?;
    let root = Directory::open(&workspace.join("payloads-v1"), true)?;
    let destination = super::staging::directory(&root, digest)?;
    let _destination = Directory::open(&destination, false)?;
    file.seek(SeekFrom::Start(archive.offset))?;
    let mut stream = GzDecoder::new(BufReader::with_capacity(65536, file.take(archive.compressed)));
    extract::extract(&mut stream, &package, &destination, || check(context))?;
    let compressed = stream.into_inner();
    if compressed.get_ref().limit() != 0 || !compressed.buffer().is_empty() {
        return Err(invalid("Unexpected data follows the compressed package"));
    }
    check(context)?;
    files::write_new(&destination.join("setup-manifest.json"), &archive.manifest)?;
    Ok(destination)
}

#[cfg(test)]
mod tests;
