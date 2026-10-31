// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reproduce sticky cancellation from an actual execution-owned native cache stream.

use super::*;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use sugarsubstitute_supervision::cache::{CachePolicy, Namespace, RuntimeCache, storage_key};

struct PausedOutput<'a> {
    inner: &'a mut dyn Write,
    point: &'a harness::CancelPoint,
    pause_at: usize,
    attempts: usize,
    cancelled_writes: &'a AtomicUsize,
}
impl Write for PausedOutput<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.attempts += 1;
        if self.attempts > self.pause_at {
            return Err(io::Error::other("Archive retried the cancelled native cache write"));
        }
        if self.attempts == self.pause_at {
            self.point.pause();
        }
        let result = self.inner.write(bytes);
        if matches!(&result, Err(cause) if cause.kind()==io::ErrorKind::Interrupted) {
            self.cancelled_writes.fetch_add(1, Ordering::SeqCst);
        }
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[test]
fn archive_cache_cancellation_stops_header_hash_and_data_writes_without_retry_or_publication()
-> harness::TestResult {
    for pause_at in 1..=3 {
        let retained = Arc::new(Mutex::new(None));
        let worker_retained = retained.clone();
        let observed = Arc::new(AtomicBool::new(false));
        let worker_observed = observed.clone();
        harness::run_storage_cancelled(24 * 1024 * 1024, move |context, point| {
            let root = tempfile::tempdir()?;
            let cache =
                RuntimeCache::open(&root.path().join("cache"), CachePolicy::default(), context)?;
            let descriptor = descriptor()?;
            let key = storage_key(&descriptor.asset().0);
            let mut lease = cache.clone().fresh(
                Namespace::ImageSources,
                &key,
                descriptor.layout().length(),
                10,
                context,
            )?;
            let target = lease.path().join("source.raster");
            *worker_retained.lock().map_err(|cause| io::Error::other(cause.to_string()))? =
                Some((root, cache, key, target.clone()));
            let interrupted = AtomicUsize::new(0);
            let mut stopped = false;
            let result =
                lease.write_with("source.raster", descriptor.layout().length(), context, |inner| {
                    let mut output = PausedOutput {
                        inner,
                        point,
                        pause_at,
                        attempts: 0,
                        cancelled_writes: &interrupted,
                    };
                    let before = context
                        .memory()
                        .claim(u64::MAX)
                        .err()
                        .ok_or_else(|| io::Error::other("Unbounded archive claim succeeded"))?
                        .available_bytes;
                    let result = archive(&mut output, context);
                    let after = context
                        .memory()
                        .claim(u64::MAX)
                        .err()
                        .ok_or_else(|| io::Error::other("Unbounded archive claim succeeded"))?
                        .available_bytes;
                    stopped = matches!(
                        &result,
                        Err(ImportError::Image(sugarsubstitute_image_ops::ImageError::Cancelled))
                    ) && output.attempts == pause_at
                        && before == after;
                    result.map_err(io::Error::other)
                });
            worker_observed.store(
                result.is_err()
                    && stopped
                    && interrupted.load(Ordering::SeqCst) == 1
                    && !target.exists(),
                Ordering::SeqCst,
            );
            Ok(())
        })?;
        assert!(observed.load(Ordering::SeqCst));
        harness::run_storage(12 * 1024 * 1024, move |context| {
            let (root, cache, key, target) = retained
                .lock()
                .map_err(|cause| io::Error::other(cause.to_string()))?
                .take()
                .ok_or(ImportError::Invalid("Cancelled archive cache custody was lost"))?;
            assert!(cache.clone().find(Namespace::ImageSources, &key, 10, context)?.is_none());
            assert!(!target.exists());
            drop(cache);
            root.close()?;
            Ok(())
        })?;
    }
    Ok(())
}
