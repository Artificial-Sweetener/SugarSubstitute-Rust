// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify retained encoded image custody, bounds and publication on the actual storage worker.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    io::{Seek, SeekFrom},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[test]
fn retained_encoded_cache_input_uses_the_same_raster_identity_without_a_source_path()
-> harness::TestResult {
    harness::run_storage(64 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let path = root.path().join("selected.png");
        let bytes = encoded(&[255, 0, 0, 0, 255, 0])?;
        std::fs::write(&path, &bytes)?;
        let cache = RuntimeCache::open(
            &root.path().join("cache"),
            CachePolicy { total_bytes: 1024 * 1024 },
            context,
        )?;
        let mut entry = cache.clone().fresh(
            Namespace::ImageSources,
            &storage_key(b"encoded output fixture"),
            bytes.len() as u64,
            10,
            context,
        )?;
        entry.write("output.png", &bytes, context)?;
        entry.publish(context)?;
        let mut encoded = entry.into_file_reader("output.png", context)?;
        let mut output = import_cached_png(
            &mut encoded,
            cache.clone(),
            CacheReuse::Compatible,
            context.memory(),
            10,
            context,
        )?;
        let original = import_png(
            &path,
            cache.clone(),
            CacheReuse::Compatible,
            context.memory(),
            10,
            context,
        )?;
        assert_eq!(output.descriptor().asset(), original.descriptor().asset());
        assert_eq!(pixels(&mut output, context)?[0], Pixel::linear(1.0, 0.0, 0.0, 1.0)?);
        assert_eq!(
            cache
                .clone()
                .maintain(10 + Namespace::ImageSources.retention(), context)?
                .pinned_entries,
            2
        );
        drop((output, original, encoded));
        assert_eq!(
            cache.maintain(10 + Namespace::ImageSources.retention(), context)?.removed_entries,
            2
        );
        assert_eq!(std::fs::read(path)?, bytes);
        Ok(())
    })
}

/// Make the final length observation differ after a complete, otherwise valid decode.
struct ChangedLength {
    bytes: Cursor<Vec<u8>>,
    end_queries: usize,
}

impl Read for ChangedLength {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.bytes.read(output)
    }
}

impl Seek for ChangedLength {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = self.bytes.seek(from)?;
        if from == SeekFrom::End(0) {
            self.end_queries += 1;
            if self.end_queries > 1 {
                return Ok(position + 1);
            }
        }
        Ok(position)
    }
}

#[test]
fn changed_encoded_input_never_publishes_a_reconstructed_generation() -> harness::TestResult {
    harness::run_storage(64 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let bytes = encoded(&[255, 0, 0, 0, 255, 0])?;
        let descriptor = RasterDescriptor::new(
            Extent::new(2, 1)?,
            Packing::Rgba8,
            Transfer::Srgb,
            Sha256::digest(&bytes).into(),
        )?;
        let cache = RuntimeCache::open(
            &root.path().join("cache"),
            CachePolicy { total_bytes: 1024 * 1024 },
            context,
        )?;
        let mut input = ChangedLength { bytes: Cursor::new(bytes), end_queries: 0 };
        let failure = crate::cached::import_encoded(
            &mut input,
            cache.clone(),
            CacheReuse::Compatible,
            context.memory(),
            10,
            context,
        )
        .err()
        .ok_or(ImportError::Invalid("Changed source was accepted"))?;
        assert!(
            matches!(failure, ImportError::Io(ref cause) if cause.kind() == io::ErrorKind::InvalidData)
        );
        assert_eq!(input.end_queries, 2);
        assert!(
            cache
                .find(Namespace::ImageSources, &storage_key(&descriptor.asset().0), 10, context)?
                .is_none()
        );
        Ok(())
    })
}

#[test]
fn malformed_retained_png_keeps_its_original_and_publishes_no_raster() -> harness::TestResult {
    harness::run_storage(64 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let mut bytes = encoded(&[255, 0, 0, 0, 255, 0])?;
        bytes.truncate(bytes.len() - 8);
        let descriptor = RasterDescriptor::new(
            Extent::new(2, 1)?,
            Packing::Rgba8,
            Transfer::Srgb,
            Sha256::digest(&bytes).into(),
        )?;
        let cache = RuntimeCache::open(
            &root.path().join("cache"),
            CachePolicy { total_bytes: 1024 * 1024 },
            context,
        )?;
        let mut entry = cache.clone().fresh(
            Namespace::ImageSources,
            &storage_key(b"malformed encoded fixture"),
            bytes.len() as u64,
            10,
            context,
        )?;
        entry.write("output.png", &bytes, context)?;
        entry.publish(context)?;
        let mut original = entry.into_file_reader("output.png", context)?;
        assert!(
            import_cached_png(
                &mut original,
                cache.clone(),
                CacheReuse::Compatible,
                context.memory(),
                10,
                context,
            )
            .is_err()
        );
        assert!(
            cache
                .find(Namespace::ImageSources, &storage_key(&descriptor.asset().0), 10, context)?
                .is_none()
        );
        original.rewind()?;
        let mut retained = Vec::new();
        original.read_to_end(&mut retained)?;
        assert_eq!(retained, bytes);
        Ok(())
    })
}

/// Expose a short stream through an incorrect native length observation.
struct TruncatedInput(Cursor<Vec<u8>>);

impl Read for TruncatedInput {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.0.read(output)
    }
}

impl Seek for TruncatedInput {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = self.0.seek(from)?;
        Ok(if from == SeekFrom::End(0) { position + 1 } else { position })
    }
}

#[test]
fn encoded_input_enforces_its_size_and_seek_bounds() -> harness::TestResult {
    harness::run_storage(1024 * 1024, |context| {
        let mut bytes = Cursor::new(vec![1, 2, 3, 4]);
        assert!(crate::cached_input::BoundedInput::new(&mut bytes, 3, context).is_err());
        let mut input = crate::cached_input::BoundedInput::new(&mut bytes, 4, context)?;
        assert_eq!(input.seek(SeekFrom::End(-1))?, 3);
        assert!(matches!(
            input.seek(SeekFrom::Current(2)),
            Err(cause) if cause.kind() == io::ErrorKind::InvalidInput
        ));
        assert_eq!(input.seek(SeekFrom::Start(0))?, 0);
        let mut output = [0; 4];
        input.read_exact(&mut output)?;
        assert_eq!(output, [1, 2, 3, 4]);
        assert_eq!(input.read(&mut output)?, 0);
        input.finish()?;
        let mut truncated = TruncatedInput(Cursor::new(vec![1, 2, 3]));
        let mut input = crate::cached_input::BoundedInput::new(&mut truncated, 4, context)?;
        assert!(matches!(
            input.read_exact(&mut output),
            Err(cause) if cause.kind() == io::ErrorKind::UnexpectedEof
        ));
        Ok(())
    })
}

struct PausingInput<'a> {
    bytes: Cursor<Vec<u8>>,
    point: &'a harness::CancelPoint,
}

impl Read for PausingInput<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.point.pause();
        self.bytes.read(output)
    }
}

impl Seek for PausingInput<'_> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(from)
    }
}

#[test]
fn encoded_input_observes_cancellation_after_an_in_flight_read() -> harness::TestResult {
    let observed = Arc::new(AtomicBool::new(false));
    let worker_observed = observed.clone();
    harness::run_storage_cancelled(1024 * 1024, move |context, point| {
        let mut source = PausingInput { bytes: Cursor::new(vec![1, 2, 3]), point };
        let mut input = crate::cached_input::BoundedInput::new(&mut source, 3, context)?;
        let mut output = [0; 3];
        let failure = input
            .read(&mut output)
            .err()
            .ok_or(ImportError::Invalid("Cancelled read completed"))?;
        worker_observed.store(failure.kind() == io::ErrorKind::Interrupted, Ordering::SeqCst);
        Ok(())
    })?;
    assert!(observed.load(Ordering::SeqCst));
    Ok(())
}
