// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Immutable byte originals retain one actual allocation and charge across clones and rebuild.
use super::*;
const JPEG: &[u8] = include_bytes!("../../tests/jpeg/fixtures/420.jpg");

#[test]
fn memory_original_failed_admission_preserves_bytes_and_claim_then_final_clone_releases_both()
-> harness::TestResult {
    harness::run_storage(1024 * 1024, |context| {
        let bytes = JPEG.to_vec();
        let capacity = bytes.capacity() as u64 + 128;
        let claim = context.memory().claim(capacity)?;
        let pointer = bytes.as_ptr();
        let mut input = Some((bytes, claim));
        let occupied = context.memory().claim(context.memory().bytes() - capacity)?;
        assert!(matches!(
            ImageSource::memory(&mut input, context.memory()),
            Err(ImportError::Image(_))
        ));
        let retained =
            input.as_ref().ok_or(ImportError::Invalid("Rejected memory original was consumed"))?;
        assert_eq!(retained.0.as_ptr(), pointer);
        assert_eq!(retained.1.bytes(), capacity);
        drop(occupied);
        let source = ImageSource::memory(&mut input, context.memory())?;
        assert!(input.is_none());
        assert!(source.requires_worker_retirement());
        let clone = source.clone();
        for original in [&source, &clone] {
            let Original::Memory(memory) = &original.inner else {
                return Err(ImportError::Invalid("Memory original changed owner"));
            };
            assert_eq!(memory.bytes()?.as_ptr(), pointer);
            assert_eq!(memory.bytes()?, JPEG);
        }
        drop(source);
        assert!(context.memory().claim(context.memory().bytes()).is_err());
        drop(clone);
        drop(context.memory().claim(context.memory().bytes())?);
        Ok(())
    })
}

#[test]
fn memory_original_rejects_underfunded_capacity_and_empty_input_without_taking_custody()
-> harness::TestResult {
    harness::run_storage(1024 * 1024, |context| {
        let mut bytes = Vec::with_capacity(4096);
        bytes.extend_from_slice(JPEG);
        let mut input = Some((bytes, context.memory().claim(1024)?));
        assert!(matches!(
            ImageSource::memory(&mut input, context.memory()),
            Err(ImportError::Invalid(_))
        ));
        assert_eq!(input.as_ref().map(|(bytes, _)| bytes.capacity()), Some(4096));
        drop(input);
        let mut empty = Some((Vec::new(), context.memory().claim(128)?));
        assert!(ImageSource::memory(&mut empty, context.memory()).is_err());
        assert!(empty.is_some());
        Ok(())
    })
}

#[test]
fn memory_original_rebuild_uses_the_same_cache_and_keeps_its_single_original_allocation()
-> harness::TestResult {
    harness::run_storage(48 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let cache = RuntimeCache::open(
            &root.path().join("cache"),
            CachePolicy { total_bytes: 1024 * 1024 },
            context,
        )?;
        let bytes = JPEG.to_vec();
        let pointer = bytes.as_ptr();
        let claim = context.memory().claim(bytes.capacity() as u64 + 128)?;
        let mut input = Some((bytes, claim));
        let original = ImageSource::memory(&mut input, context.memory())?;
        let clone = original.clone();
        let first = original.import_image(
            cache.clone(),
            CacheReuse::Compatible,
            context.memory(),
            10,
            context,
        )?;
        let second = clone.import_image(
            cache.clone(),
            CacheReuse::Rebuild,
            context.memory(),
            11,
            context,
        )?;
        assert_eq!(first.descriptor(), second.descriptor());
        let Original::Memory(memory) = &clone.inner else {
            return Err(ImportError::Invalid("Memory original changed owner"));
        };
        assert_eq!(memory.bytes()?.as_ptr(), pointer);
        assert_eq!(
            cache.maintain(12 + Namespace::ImageSources.retention(), context)?.pinned_entries,
            2
        );
        drop((first, second, original, clone));
        Ok(())
    })
}
