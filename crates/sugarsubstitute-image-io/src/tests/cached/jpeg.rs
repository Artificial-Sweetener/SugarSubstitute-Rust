// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! JPEG originals use the existing source cache and original execution admission.
use super::*;
use sha2::{Digest, Sha256};
use std::io::Seek;
mod controls;

#[test]
fn actual_comfy_sequential_jpeg_imports_through_the_existing_cache_owner() -> harness::TestResult {
    harness::run_storage(48 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let cache = RuntimeCache::open(
            &root.path().join("cache"),
            CachePolicy { total_bytes: 1024 * 1024 },
            context,
        )?;
        let bytes = include_bytes!("../jpeg/fixtures/420.jpg");
        let mut source = crate::cached::import_encoded(
            &mut Cursor::new(bytes.as_slice()),
            cache,
            CacheReuse::Compatible,
            context.memory(),
            10,
            context,
        )?;
        let descriptor = source.descriptor();
        assert_eq!(descriptor.layout().extent(), Extent::new(19, 17)?);
        let mut pixel = [Pixel::TRANSPARENT];
        source.read(descriptor.asset(), Region::new(18, 16, 1, 1)?, &mut pixel, context)?;
        assert_eq!(pixel[0].channels()[3], 1.0);
        Ok(())
    })
}

#[test]
fn jpeg_retained_original_clones_rebuild_the_same_asset_without_mutating_prior_generations()
-> harness::TestResult {
    harness::run_storage(48 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let bytes = crate::tests::jpeg::FIXTURES[3];
        let cache = RuntimeCache::open(
            &root.path().join("cache"),
            CachePolicy { total_bytes: 1024 * 1024 },
            context,
        )?;
        let mut entry = cache.clone().fresh(
            Namespace::ComfyOutputs,
            &storage_key(b"retained JPEG original"),
            bytes.len() as u64,
            10,
            context,
        )?;
        entry.write("output.jpg", bytes, context)?;
        entry.publish(context)?;
        let mut input = Some(entry.into_file_reader("output.jpg", context)?);
        let original = ImageSource::encoded(&mut input, context.memory())?;
        assert!(input.is_none());
        let cloned = original.clone();
        let first = original.import_image(
            cache.clone(),
            CacheReuse::Compatible,
            context.memory(),
            10,
            context,
        )?;
        drop(original);
        let rebuilt = cloned.import_image(
            cache.clone(),
            CacheReuse::Rebuild,
            context.memory(),
            11,
            context,
        )?;
        assert_eq!(first.descriptor(), rebuilt.descriptor());
        assert_eq!(rebuilt.descriptor().original_hash(), Sha256::digest(bytes).as_slice());
        let report = cache.clone().maintain(12 + Namespace::ImageSources.retention(), context)?;
        assert_eq!(report.pinned_entries, 3);
        drop((first, rebuilt));
        let reused = cloned.import_image(
            cache.clone(),
            CacheReuse::Compatible,
            context.memory(),
            12,
            context,
        )?;
        assert_eq!(reused.descriptor().original_hash(), Sha256::digest(bytes).as_slice());
        drop((cloned, reused));
        assert_eq!(
            cache.maintain(13 + Namespace::ImageSources.retention(), context)?.removed_entries,
            3
        );
        Ok(())
    })
}

#[test]
fn malformed_jpeg_keeps_its_encoded_cache_original_and_publishes_no_raster() -> harness::TestResult
{
    harness::run_storage(48 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let mut bytes = crate::tests::jpeg::FIXTURES[3].to_vec();
        bytes.truncate(bytes.len() - 2);
        let descriptor = RasterDescriptor::new(
            Extent::new(19, 17)?,
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
            Namespace::ComfyOutputs,
            &storage_key(b"malformed JPEG original"),
            bytes.len() as u64,
            10,
            context,
        )?;
        entry.write("output.jpg", &bytes, context)?;
        entry.publish(context)?;
        let mut original = entry.into_file_reader("output.jpg", context)?;
        assert!(
            import_cached_image(
                &mut original,
                cache.clone(),
                CacheReuse::Compatible,
                context.memory(),
                10,
                context
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
