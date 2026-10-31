// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Cancel original metadata validation without publishing an incomplete JPEG generation.
use super::*;
use std::{
    io::{Read, SeekFrom},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

struct MetadataInput<'a> {
    bytes: Cursor<&'static [u8]>,
    end_queries: usize,
    point: &'a harness::CancelPoint,
}
impl Read for MetadataInput<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.bytes.read(output)
    }
}
impl Seek for MetadataInput<'_> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        if from == SeekFrom::End(0) {
            self.end_queries += 1;
            // The encoded-input owner measures first; the image adapter measures after hashing.
            if self.end_queries == 2 {
                self.point.pause();
            }
        }
        self.bytes.seek(from)
    }
}

#[test]
fn jpeg_cancelled_metadata_validation_publishes_no_derived_cache_generation() -> harness::TestResult
{
    let retained = Arc::new(Mutex::new(None));
    let worker_retained = retained.clone();
    let observed = Arc::new(AtomicBool::new(false));
    let worker_observed = observed.clone();
    harness::run_storage_cancelled(48 * 1024 * 1024, move |context, point| {
        let root = tempfile::tempdir()?;
        let bytes = crate::tests::jpeg::FIXTURES[3];
        let descriptor = RasterDescriptor::new(
            Extent::new(19, 17)?,
            Packing::Rgba8,
            Transfer::Srgb,
            Sha256::digest(bytes).into(),
        )?;
        let cache = RuntimeCache::open(
            &root.path().join("cache"),
            CachePolicy { total_bytes: 1024 * 1024 },
            context,
        )?;
        *worker_retained.lock().map_err(|cause| io::Error::other(cause.to_string()))? =
            Some((root, cache.clone(), descriptor));
        let mut input = MetadataInput { bytes: Cursor::new(bytes), end_queries: 0, point };
        worker_observed.store(
            crate::cached::import_encoded(
                &mut input,
                cache,
                CacheReuse::Compatible,
                context.memory(),
                10,
                context,
            )
            .is_err(),
            Ordering::SeqCst,
        );
        Ok(())
    })?;
    assert!(observed.load(Ordering::SeqCst));
    harness::run_storage(16 * 1024 * 1024, move |context| {
        let (root, cache, descriptor) = retained
            .lock()
            .map_err(|cause| io::Error::other(cause.to_string()))?
            .take()
            .ok_or(ImportError::Invalid("Cancelled cache fixture was not retained"))?;
        assert!(
            cache
                .find(Namespace::ImageSources, &storage_key(&descriptor.asset().0), 10, context)?
                .is_none()
        );
        drop(root);
        Ok(())
    })
}
