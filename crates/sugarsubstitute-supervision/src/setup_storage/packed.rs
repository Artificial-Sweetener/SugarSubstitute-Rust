// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode the common native archive through installer trust or retained authenticated download custody.
use super::VerifiedDownload;
use super::bundle::Payload;
use super::cache;
use super::cache::Namespace;
use super::cache::RuntimeCache;
use super::check;
use super::invalid;
use crate::durable_storage::files::{self, Directory};
use flate2::bufread::GzDecoder;
use sha2::{Digest, Sha256};
use std::{
    io::{self, BufReader, Read, Seek, SeekFrom},
    path::Path,
};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_contracts::update::Component;
use sugarsubstitute_update::package::{MAX_MANIFEST_BYTES, Manifest};
const MAGIC: &[u8; 8] = b"SSSETUP1";
/// Storage admission for materialization, retained scope/entry metadata and authenticated files.
/// Later copy operations require their own reservation alongside this retained source admission.
pub const MATERIALIZATION_RESERVATION_BYTES: u64 = 32 * 1024 * 1024;
const ARCHIVE_MEMORY: u64 = 2 * 1024 * 1024;
const EXTRACTION_MEMORY: u64 = 14 * 1024 * 1024;
mod extract;
mod footer;

/// Resolve a development bundle or unpack the compiled installer's authenticated inventory.
/// Extraction runs on an admitted storage worker and never executes a payload.
/// # Errors
/// Rejects malformed archives, untrusted inventories, traversal and changed payload bytes.
pub fn materialize(
    source: &Path,
    cache: &RuntimeCache,
    expected: &str,
    context: &JobContext,
) -> io::Result<super::VerifiedBundle> {
    check(context)?;
    if source.is_dir() {
        return super::VerifiedBundle::open(source, expected, context);
    }
    let _memory = context.memory().claim(ARCHIVE_MEMORY).map_err(io::Error::other)?;
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
    Ok(super::VerifiedBundle {
        payload: unpack(&mut file, archive, cache, expected, Component::Application, context)?,
    })
}

/// Authenticate the inventory through the exact verified archive handle, without reopening a path.
/// Bare update archives use the same format as installer payloads, with no executable envelope.
pub(super) fn downloaded(
    source: &mut VerifiedDownload,
    cache: &RuntimeCache,
    component: Component,
    context: &JobContext,
) -> io::Result<Payload> {
    check(context)?;
    let _memory = context.memory().claim(ARCHIVE_MEMORY).map_err(io::Error::other)?;
    let archive = read_inventory(source)?;
    if archive.offset != 0 || archive.end != source.identity().length() {
        return Err(invalid("Update package has an unexpected executable envelope"));
    }
    let digest = format!("{:x}", Sha256::digest(&archive.manifest));
    unpack(source, archive, cache, &digest, component, context)
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
    cache: &RuntimeCache,
    digest: &str,
    component: Component,
    context: &JobContext,
) -> io::Result<Payload> {
    check(context)?;
    let component_name = match component {
        Component::Application => "application",
        Component::Updater => "updater",
    };
    let key = cache::storage_key(format!("payload-v1:{component_name}:{digest}").as_bytes());
    let now = cache::utc_seconds()?;
    if let Some(lease) = cache.clone().find(Namespace::PackedPayloads, &key, now, context)? {
        {
            let _verification =
                context.memory().claim(EXTRACTION_MEMORY).map_err(io::Error::other)?;
            let package = Manifest::decode(&archive.manifest).map_err(io::Error::other)?;
            super::inventory::require_component(&package, component)?;
            file.seek(SeekFrom::Start(archive.offset))?;
            let mut stream =
                GzDecoder::new(BufReader::with_capacity(65536, file.take(archive.compressed)));
            extract::verify(&mut stream, &package, || check(context))?;
            let compressed = stream.into_inner();
            if compressed.get_ref().limit() != 0 || !compressed.buffer().is_empty() {
                return Err(invalid("Unexpected data follows the compressed package"));
            }
        }
        let path = lease.path();
        match Payload::open_pinned(
            &path,
            digest,
            component,
            Some(std::sync::Arc::new(lease)),
            context,
        ) {
            Ok(payload) => return Ok(payload),
            Err(cause)
                if matches!(
                    cause.kind(),
                    io::ErrorKind::InvalidInput
                        | io::ErrorKind::InvalidData
                        | io::ErrorKind::NotFound
                        | io::ErrorKind::PermissionDenied
                ) =>
            {
                tracing::warn!(operation="payload_cache_miss", package=%digest, cause=%cause, "Preserving rejected extracted cache bytes");
            }
            Err(cause) => return Err(cause),
        }
    }
    let lease = {
        let _extraction = context.memory().claim(EXTRACTION_MEMORY).map_err(io::Error::other)?;
        let package = Manifest::decode(&archive.manifest).map_err(io::Error::other)?;
        super::inventory::require_component(&package, component)?;
        let maximum =
            package.entries().iter().try_fold(archive.manifest.len() as u64, |total, entry| {
                total
                    .checked_add(entry.bytes())
                    .ok_or_else(|| invalid("Extracted cache reservation overflowed"))
            })?;
        let mut lease =
            cache.clone().fresh(Namespace::PackedPayloads, &key, maximum, now, context)?;
        lease.declare(
            package
                .entries()
                .iter()
                .map(|entry| entry.path().to_owned())
                .chain(std::iter::once("setup-manifest.json".into())),
            context,
        )?;
        let destination = lease.path();
        file.seek(SeekFrom::Start(archive.offset))?;
        let mut stream =
            GzDecoder::new(BufReader::with_capacity(65536, file.take(archive.compressed)));
        extract::extract(&mut stream, &package, &destination, context, || check(context))?;
        let compressed = stream.into_inner();
        if compressed.get_ref().limit() != 0 || !compressed.buffer().is_empty() {
            return Err(invalid("Unexpected data follows the compressed package"));
        }
        check(context)?;
        files::write_new(&destination.join("setup-manifest.json"), &archive.manifest)?;
        lease.verify_storage(context)?;
        lease.publish(context)?;
        lease.into_reader(context)?
    };
    let path = lease.path();
    Payload::open_pinned(&path, digest, component, Some(std::sync::Arc::new(lease)), context)
}

#[cfg(test)]
mod tests;
