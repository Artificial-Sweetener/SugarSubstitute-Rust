// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Import selected original files through existing storage admission and retained cache custody.
use crate::{ImportError, PngImport, RasterArchive, buffer::Buffer};
use sha2::{Digest, Sha256};
use std::{
    io::{self, BufReader, Read, Seek},
    path::Path,
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_supervision::{
    ArtifactError,
    cache::{CacheReader, Namespace, RuntimeCache, storage_key},
    with_artifact_input,
};

const MAX_INPUT: u64 = 32 * 1024 * 1024 * 1024;
const INPUT_BUFFER: usize = 64 * 1024;
const SOURCE_FILE: &str = "source.raster";

/// Complete immutable original raster with native file and shared-cache pin retained together.
pub type CachedRaster = RasterArchive<CacheReader<'static>>;
/// Explicit recovery policy selected by the application after a lazy cache-integrity failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheReuse {
    /// Reuse compatible metadata and validate each requested source chunk before use.
    Compatible,
    /// Reimport original bytes into a fresh generation, preserving every preceding generation.
    Rebuild,
}
/// Import a caller-selected PNG without retaining an image-sized decoded allocation.
/// Run on an execution-owned storage worker with at least 48 MiB transient admission.
/// `resident` is a separate original retained lease for source tables/workspace; `cache` owns
/// its own original entry admission. Originals are opened read-only and remain outside cache.
/// Compatible entries validate each chunk lazily; a later `ImportError::Corrupt` asks the
/// application to schedule `Rebuild`, never to overwrite or discard authored pixels.
/// # Errors
/// Preserves original acquisition, decode, disk/memory admission, controls and native failures.
pub fn import_png(
    path: &Path,
    cache: RuntimeCache,
    reuse: CacheReuse,
    resident: &MemoryLease,
    now: u64,
    context: &JobContext,
) -> Result<CachedRaster, ImportError> {
    if context.deadline().is_none() {
        return Err(ImportError::Invalid(
            "Image import requires an original bounded storage deadline",
        ));
    }
    with_artifact_input(path, MAX_INPUT, context, |input| {
        let content = hash_original(input, context)?;
        input.rewind()?;
        let _buffer = context.memory().claim(INPUT_BUFFER as u64 + 128)?;
        let import = PngImport::open(BufReader::with_capacity(INPUT_BUFFER, input), content, context)?;
        let descriptor = import.descriptor();
        let key = storage_key(&descriptor.asset().0);
        if reuse == CacheReuse::Compatible
            && let Some(entry) = cache.clone().find(Namespace::ImageSources, &key, now, context)? {
            let opened = entry.into_file_reader(SOURCE_FILE, context)
                .map_err(ImportError::Io)
                .and_then(|reader| RasterArchive::open(reader, descriptor, resident, context));
            match opened {
                Ok(source) => return Ok(source),
                Err(ImportError::Corrupt(cause)) => {
                    tracing::warn!(operation="image_source_cache_miss", source_key=%key, cause=%cause, "Preserving corrupt reconstructed image generation");
                }
                Err(ImportError::Io(cause)) if matches!(cause.kind(), io::ErrorKind::NotFound | io::ErrorKind::InvalidData) => {
                    tracing::warn!(operation="image_source_cache_miss", source_key=%key, cause=%cause, "Preserving incomplete reconstructed image generation");
                }
                Err(cause) => return Err(cause),
            }
        }
        let mut entry = cache.fresh(Namespace::ImageSources, &key, descriptor.layout().length(), now, context)?;
        entry.write_with(SOURCE_FILE, descriptor.layout().length(), context, |output| {
            import.write(output, context).map_err(io::Error::other)
        })?;
        crate::check(context)?;
        entry.publish(context)?;
        let reader = entry.into_file_reader(SOURCE_FILE, context)?;
        RasterArchive::open(reader, descriptor, resident, context)
    }).map_err(|cause| match cause {
        ArtifactError::Storage(cause) => ImportError::Io(cause),
        ArtifactError::Producer(cause) => cause,
        ArtifactError::Retirement { producer, storage } => {
            ImportError::Io(io::Error::other(ArtifactError::Retirement { producer, storage }))
        }
    })
}

/// Hash the complete original stream using bounded admitted storage-worker input workspace.
fn hash_original(input: &mut impl Read, context: &JobContext) -> Result<[u8; 32], ImportError> {
    let mut bytes = Buffer::new(INPUT_BUFFER, 0, context.memory())?;
    let mut hash = Sha256::new();
    loop {
        crate::check(context)?;
        let read = input.read(&mut bytes.data)?;
        if read == 0 {
            break;
        }
        hash.update(&bytes.data[..read]);
    }
    Ok(hash.finalize().into())
}
