// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep read handles coupled to their original borrowed or transferred entry pins.
use super::*;
use std::io::{Read, Seek, SeekFrom};

/// A regular read-only cache file that cannot outlive its original entry pin.
/// Feature owners authenticate its bytes before use. Buffering remains charged to their job.
pub struct CacheReader<'a> {
    file: File,
    _pin: Pin<'a>,
}
enum Pin<'a> {
    Borrowed { _lease: &'a CacheLease },
    Owned { _lease: std::sync::Arc<CacheLease> },
}
impl<'a> CacheReader<'a> {
    /// Tie the opened regular file to the exact entry owner that established its custody.
    pub(super) fn new(file: File, lease: &'a CacheLease) -> Self {
        Self { file, _pin: Pin::Borrowed { _lease: lease } }
    }
    /// Retain a transferred entry pin after opening exactly its declared regular file.
    pub(super) fn owned(file: File, lease: CacheLease) -> CacheReader<'static> {
        CacheReader { file, _pin: Pin::Owned { _lease: std::sync::Arc::new(lease) } }
    }
    /// Read bounded metadata with the same before/after length checks as native storage inputs.
    /// The caller must admit `maximum` bytes of buffering before invoking this operation.
    /// # Errors
    /// Rejects oversized content and preserves native read or metadata failures.
    pub fn read_bounded(&mut self, maximum: usize) -> io::Result<Vec<u8>> {
        files::read_bounded(&mut self.file, maximum)
    }
}
impl Read for CacheReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.file.read(output)
    }
}
impl Seek for CacheReader<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.file.seek(position)
    }
}
