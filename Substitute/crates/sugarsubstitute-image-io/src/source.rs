// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the exact original input needed to reconstruct a displayed raster.

use crate::{CacheReuse, CachedRaster, ImportError, import_cached_image, import_image};
use std::{
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::{Arc, Mutex, TryLockError},
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryLease};
use sugarsubstitute_supervision::cache::{CacheReader, RuntimeCache};
mod memory;
use memory::MemoryOriginal;

#[cfg(test)]
mod tests;

/// Encoded storage remains pinned across imports and cache reconstruction.
struct Encoded {
    reader: Mutex<CacheReader<'static>>,
    _memory: MemoryClaim,
}

/// An explicit local file, pinned encoded cache stream or already-admitted immutable bytes.
///
/// Clones retain the same original, including its native cache pin. Import and final release
/// of a retained stream belong on admitted storage workers, never the native UI thread.
/// This storage custody does not authorize a backend, network destination or web request.
#[derive(Clone)]
pub struct ImageSource {
    inner: Original,
}

#[derive(Clone)]
enum Original {
    File(PathBuf),
    Encoded(Arc<Encoded>),
    Memory(MemoryOriginal),
}

impl ImageSource {
    /// Retain an explicitly selected local path; acquisition and validation occur on import.
    pub fn file(path: PathBuf) -> Self {
        Self { inner: Original::File(path) }
    }

    /// Retain a caller-owned encoded stream without accepting or reopening a path.
    ///
    /// Admission happens before transferring the original. On failure, `input` remains intact.
    /// The caller must create this value and retire its final clone on storage workers.
    ///
    /// # Errors
    /// Preserves an absent input or the original retained-memory admission failure.
    pub fn encoded(
        input: &mut Option<CacheReader<'static>>,
        memory: &MemoryLease,
    ) -> Result<Self, ImportError> {
        if input.is_none() {
            return Err(ImportError::Invalid("Encoded original is unavailable"));
        }
        let claim = memory.claim(size_of::<Encoded>() as u64 + 128)?;
        let reader = input.take().ok_or(ImportError::Invalid("Encoded original is unavailable"))?;
        Ok(Self {
            inner: Original::Encoded(Arc::new(Encoded {
                reader: Mutex::new(reader),
                _memory: claim,
            })),
        })
    }

    /// Transfer already-admitted encoded bytes without copying or replacing their original charge.
    ///
    /// `input` retains its Vec and claim unchanged on validation or metadata-admission failure.
    /// The transferred claim must cover actual Vec capacity plus 128 bytes; `resident` admits
    /// shared metadata separately. Clones share immutable bytes. Import, rebuild and final
    /// disposal belong on execution-owned storage workers, including memory originals.
    /// # Errors
    /// Rejects absent, empty, oversized or underfunded input; preserves metadata admission failure.
    pub fn memory(
        input: &mut Option<(Vec<u8>, MemoryClaim)>,
        resident: &MemoryLease,
    ) -> Result<Self, ImportError> {
        Ok(Self { inner: Original::Memory(MemoryOriginal::new(input, resident)?) })
    }

    /// Whether final original disposal must run on an admitted worker.
    pub fn requires_worker_retirement(&self) -> bool {
        matches!(&self.inner, Original::Encoded(_) | Original::Memory(_))
    }

    /// Import this original through the same hashing and derived-cache policy.
    ///
    /// Call on an execution-owned storage worker. Concurrent imports of the same retained
    /// stream are rejected without waiting; each caller retains its own original deadline.
    ///
    /// # Errors
    /// Returns `ImportError::Busy` without waiting for another import of the same stream.
    /// Preserves acquisition, image, admission, cancellation and native storage failures.
    pub fn import_image(
        &self,
        cache: RuntimeCache,
        reuse: CacheReuse,
        resident: &MemoryLease,
        now: u64,
        context: &JobContext,
    ) -> Result<CachedRaster, ImportError> {
        crate::check(context)?;
        match &self.inner {
            Original::File(path) => import_image(path, cache, reuse, resident, now, context),
            Original::Memory(original) => crate::cached::import_encoded(
                &mut std::io::Cursor::new(original.bytes()?),
                cache,
                reuse,
                resident,
                now,
                context,
            ),
            Original::Encoded(encoded) => {
                let mut reader = encoded.reader.try_lock().map_err(|cause| match cause {
                    TryLockError::WouldBlock => ImportError::Busy,
                    TryLockError::Poisoned(_) => {
                        ImportError::Invalid("Encoded original reader failed")
                    }
                })?;
                import_cached_image(&mut reader, cache, reuse, resident, now, context)
            }
        }
    }

    /// Copy the complete original encoded bytes, as acquired, for handing to another program.
    ///
    /// Call on an execution-owned storage worker. The returned claim, taken from `memory`,
    /// covers the copy's capacity. A retained stream already in use by an import is rejected
    /// without waiting.
    ///
    /// # Errors
    /// Rejects an empty original or one above `maximum` bytes before allocating; returns
    /// `ImportError::Busy` for a stream in use and preserves admission, cancellation and native
    /// read failures.
    pub fn encoded_bytes(
        &self,
        maximum: u64,
        memory: &MemoryLease,
        context: &JobContext,
    ) -> Result<(Vec<u8>, MemoryClaim), ImportError> {
        crate::check(context)?;
        match &self.inner {
            Original::File(path) => {
                sugarsubstitute_supervision::with_artifact_input(path, maximum, context, |input| {
                    copy_bounded(input, maximum, memory, context)
                })
                .map_err(|cause| match cause {
                    sugarsubstitute_supervision::ArtifactError::Storage(cause) => {
                        ImportError::Io(cause)
                    }
                    sugarsubstitute_supervision::ArtifactError::Producer(cause) => cause,
                    retirement => ImportError::Io(std::io::Error::other(retirement)),
                })
            }
            Original::Memory(original) => {
                let bytes = original.bytes()?;
                copy_bounded(&mut std::io::Cursor::new(bytes), maximum, memory, context)
            }
            Original::Encoded(encoded) => {
                let mut reader = encoded.reader.try_lock().map_err(|cause| match cause {
                    TryLockError::WouldBlock => ImportError::Busy,
                    TryLockError::Poisoned(_) => {
                        ImportError::Invalid("Encoded original reader failed")
                    }
                })?;
                copy_bounded(&mut *reader, maximum, memory, context)
            }
        }
    }
}

/// Read a complete seekable stream after admitting its exact length within `maximum` bytes.
fn copy_bounded(
    stream: &mut (impl Read + Seek),
    maximum: u64,
    memory: &MemoryLease,
    context: &JobContext,
) -> Result<(Vec<u8>, MemoryClaim), ImportError> {
    let length = stream.seek(SeekFrom::End(0))?;
    if length == 0 || length > maximum {
        return Err(ImportError::Invalid("Encoded original length is outside its limit"));
    }
    let length = usize::try_from(length)
        .map_err(|_| ImportError::Invalid("Encoded original length is outside its limit"))?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length)?;
    let claim = memory.claim(bytes.capacity() as u64)?;
    bytes.resize(length, 0);
    stream.seek(SeekFrom::Start(0))?;
    crate::check(context)?;
    stream.read_exact(&mut bytes)?;
    Ok((bytes, claim))
}
