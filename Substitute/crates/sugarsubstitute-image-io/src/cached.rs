// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Import selected original files through existing storage admission and retained cache custody.
use crate::{
    ImportError, MAX_ENCODED_SOURCE_BYTES, RasterArchive, RasterDescriptor, buffer::Buffer,
    image_input::ImageImport,
};
use sha2::{Digest, Sha256};
use std::{
    io::{self, BufReader, Read, Seek},
    path::Path,
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_supervision::{
    ArtifactError,
    cache::{CacheLease, CacheReader, Namespace, RuntimeCache, storage_key},
    with_artifact_input,
};

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
/// Import a caller-selected supported image through original storage and cache custody.
/// Run on an execution-owned storage worker with at least 48 MiB transient admission.
/// PNG streams scanlines; JPEG additionally admits its full RGB frame before decoding.
/// `resident` is a separate original retained lease for source tables/workspace; `cache` owns
/// its own original entry admission. Originals are opened read-only and remain outside cache.
/// Compatible entries validate each chunk lazily; a later `ImportError::Corrupt` asks the
/// application to schedule `Rebuild`, never to overwrite or discard authored pixels.
/// # Errors
/// Preserves original acquisition, decode, disk/memory admission, controls and native failures.
pub fn import_image(
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
    with_artifact_input(path, MAX_ENCODED_SOURCE_BYTES, context, |input| {
        let prepared = prepare_stream(input, cache, reuse, resident, now, context)?;
        prepared.publish(resident, context)
    })
    .map_err(|cause| match cause {
        ArtifactError::Storage(cause) => ImportError::Io(cause),
        ArtifactError::Producer(cause) => cause,
        ArtifactError::Retirement { producer, storage } => {
            ImportError::Io(io::Error::other(ArtifactError::Retirement { producer, storage }))
        }
    })
}

/// Import a supported image from an already-pinned, read-only cache file without accepting a path.
///
/// The caller retains the encoded input pin through this operation and any later rebuild.
/// Run on an execution-owned storage worker with at least 48 MiB transient admission.
/// The input is bounded and hashed independently; cache custody does not authenticate content.
/// `resident` retains the returned raster's source tables and cache-entry admission.
///
/// # Errors
/// Rejects oversized, changed, truncated or malformed input, cancellation and expired work;
/// preserves decoding, admission, storage and final source-validation failures.
pub fn import_cached_image(
    input: &mut CacheReader<'_>,
    cache: RuntimeCache,
    reuse: CacheReuse,
    resident: &MemoryLease,
    now: u64,
    context: &JobContext,
) -> Result<CachedRaster, ImportError> {
    import_encoded(input, cache, reuse, resident, now, context)
}

/// Check the complete input before a newly reconstructed cache generation becomes visible.
pub(super) fn import_encoded(
    input: &mut (impl Read + Seek),
    cache: RuntimeCache,
    reuse: CacheReuse,
    resident: &MemoryLease,
    now: u64,
    context: &JobContext,
) -> Result<CachedRaster, ImportError> {
    let mut bounded =
        crate::cached_input::BoundedInput::new(input, MAX_ENCODED_SOURCE_BYTES, context)?;
    let result = prepare_stream(&mut bounded, cache, reuse, resident, now, context);
    let checked = bounded.finish();
    match (result, checked) {
        (Ok(prepared), Ok(())) => prepared.publish(resident, context),
        (Ok(_), Err(storage)) => Err(ImportError::Io(storage)),
        (Err(producer), Ok(())) => Err(producer),
        (Err(producer), Err(storage)) => {
            Err(ImportError::Io(io::Error::other(ArtifactError::Retirement { producer, storage })))
        }
    }
}

/// Keep an existing generation or unpublished reconstruction under worker custody.
enum PreparedRaster {
    Existing(CachedRaster),
    New { entry: CacheLease, descriptor: RasterDescriptor },
}

impl PreparedRaster {
    /// Publish only after the input owner has confirmed its original constraints.
    fn publish(
        self,
        resident: &MemoryLease,
        context: &JobContext,
    ) -> Result<CachedRaster, ImportError> {
        match self {
            Self::Existing(source) => Ok(source),
            Self::New { mut entry, descriptor } => {
                crate::check(context)?;
                entry.publish(context)?;
                let reader = entry.into_file_reader(SOURCE_FILE, context)?;
                RasterArchive::open(reader, descriptor, resident, context)
            }
        }
    }
}

/// Reuse one hashing, decoding and cache-generation policy for every admitted input owner.
fn prepare_stream(
    input: &mut (impl Read + Seek),
    cache: RuntimeCache,
    reuse: CacheReuse,
    resident: &MemoryLease,
    now: u64,
    context: &JobContext,
) -> Result<PreparedRaster, ImportError> {
    let content = hash_original(input, context)?;
    input.rewind()?;
    let _buffer = context.memory().claim(INPUT_BUFFER as u64 + 128)?;
    let import =
        ImageImport::open(BufReader::with_capacity(INPUT_BUFFER, input), content, context)?;
    let descriptor = import.descriptor();
    let key = storage_key(&descriptor.asset().0);
    if reuse == CacheReuse::Compatible
        && let Some(entry) = cache.clone().find(Namespace::ImageSources, &key, now, context)?
    {
        let opened = entry
            .into_file_reader(SOURCE_FILE, context)
            .map_err(ImportError::Io)
            .and_then(|reader| RasterArchive::open(reader, descriptor, resident, context));
        match opened {
            Ok(source) => return Ok(PreparedRaster::Existing(source)),
            Err(ImportError::Corrupt(cause)) => {
                tracing::warn!(operation="image_source_cache_miss", source_key=%key, cause=%cause, "Preserving corrupt reconstructed image generation");
            }
            Err(ImportError::Io(cause))
                if matches!(cause.kind(), io::ErrorKind::NotFound | io::ErrorKind::InvalidData) =>
            {
                tracing::warn!(operation="image_source_cache_miss", source_key=%key, cause=%cause, "Preserving incomplete reconstructed image generation");
            }
            Err(cause) => return Err(cause),
        }
    }
    let mut entry =
        cache.fresh(Namespace::ImageSources, &key, descriptor.layout().length(), now, context)?;
    entry.write_with(SOURCE_FILE, descriptor.layout().length(), context, |output| {
        import.write(output, context).map_err(io::Error::other)
    })?;
    Ok(PreparedRaster::New { entry, descriptor })
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
