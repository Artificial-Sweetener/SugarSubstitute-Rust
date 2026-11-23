// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode an image held only for a moment, such as a live preview frame, straight into memory.
//!
//! A transient image is shown once and replaced; storing it in the image cache would write and
//! hash it on disk for nothing and fill the cache with entries no one reuses. The decoded archive
//! lives in memory, funded by its own claim, for as long as its reader is kept.
use crate::{ImportError, RasterArchive, image_input::ImageImport};
use sha2::{Digest, Sha256};
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryLease};

/// A decoded raster archive held in memory, with the claim that funds its bytes.
pub struct MemoryRaster {
    bytes: Cursor<Vec<u8>>,
    _claim: MemoryClaim,
}

impl Read for MemoryRaster {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.bytes.read(buffer)
    }
}

impl Seek for MemoryRaster {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

/// A raster archive decoded into memory, outside the image cache.
pub type TransientRaster = RasterArchive<MemoryRaster>;

/// Decode `encoded` (PNG or JPEG) into an in-memory archive, admitting its bytes from `resident`.
pub(crate) fn import_transient(
    encoded: &[u8],
    resident: &MemoryLease,
    context: &JobContext,
) -> Result<TransientRaster, ImportError> {
    crate::check(context)?;
    let content: [u8; 32] = Sha256::digest(encoded).into();
    let import = ImageImport::open(Cursor::new(encoded), content, context)?;
    let descriptor = import.descriptor();
    let length = descriptor.layout().length();
    let claim = resident.claim(length.saturating_add(128))?;
    let mut archive = Vec::new();
    archive.try_reserve_exact(
        usize::try_from(length)
            .map_err(|_| ImportError::Invalid("Transient raster exceeds addressable memory"))?,
    )?;
    import.write(&mut archive, context)?;
    RasterArchive::open(
        MemoryRaster { bytes: Cursor::new(archive), _claim: claim },
        descriptor,
        resident,
        context,
    )
}
