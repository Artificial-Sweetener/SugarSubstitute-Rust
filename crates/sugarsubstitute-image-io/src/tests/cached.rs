// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Original image identity and derived generations stay separate under real native cache custody.
use super::*;
use std::io::Read;
mod encoded;
mod jpeg;
mod source;
use sugarsubstitute_supervision::cache::{CachePolicy, Namespace, RuntimeCache, storage_key};

fn encoded(data: &[u8]) -> Result<Vec<u8>, ImportError> {
    let mut bytes = Vec::new();
    {
        let mut encoder = ::png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(::png::ColorType::Rgb);
        let mut writer = encoder.write_header().map_err(io::Error::other)?;
        writer.write_image_data(data).map_err(io::Error::other)?;
    }
    Ok(bytes)
}
fn pixels(
    source: &mut CachedRaster,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<[Pixel; 2], ImportError> {
    let descriptor = source.descriptor();
    let mut result = [Pixel::TRANSPARENT; 2];
    source.read(descriptor.asset(), descriptor.layout().extent().region(), &mut result, context)?;
    Ok(result)
}

#[test]
fn selected_original_remains_authored_while_cache_readers_reuse_and_retain_one_generation()
-> harness::TestResult {
    harness::run_storage(64 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let path = root.path().join("original.png");
        let bytes = encoded(&[255, 0, 0, 0, 255, 0])?;
        std::fs::write(&path, &bytes)?;
        let cache = RuntimeCache::open(
            &root.path().join("cache"),
            CachePolicy { total_bytes: 1024 * 1024 },
            context,
        )?;
        let mut first = import_image(
            &path,
            cache.clone(),
            CacheReuse::Compatible,
            context.memory(),
            10,
            context,
        )?;
        let second = import_image(
            &path,
            cache.clone(),
            CacheReuse::Compatible,
            context.memory(),
            10,
            context,
        )?;
        assert_eq!(first.descriptor().asset(), second.descriptor().asset());
        let report = cache.clone().maintain(10 + Namespace::ImageSources.retention(), context)?;
        assert_eq!(report.pinned_entries, 1);
        assert_eq!(report.removed_entries, 0);
        assert_eq!(pixels(&mut first, context)?[0], Pixel::linear(1.0, 0.0, 0.0, 1.0)?);
        assert_eq!(std::fs::read(&path)?, bytes);
        drop((first, second));
        assert_eq!(
            cache.maintain(10 + Namespace::ImageSources.retention(), context)?.removed_entries,
            1
        );
        assert_eq!(std::fs::read(path)?, bytes);
        Ok(())
    })
}

#[test]
fn replacing_an_original_path_cannot_mutate_an_already_pinned_source_asset() -> harness::TestResult
{
    harness::run_storage(64 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let path = root.path().join("original.png");
        std::fs::write(&path, encoded(&[255, 0, 0, 0, 255, 0])?)?;
        let cache = RuntimeCache::open(
            &root.path().join("cache"),
            CachePolicy { total_bytes: 1024 * 1024 },
            context,
        )?;
        let mut first = import_image(
            &path,
            cache.clone(),
            CacheReuse::Compatible,
            context.memory(),
            10,
            context,
        )?;
        std::fs::write(&path, encoded(&[0, 0, 255, 255, 255, 255])?)?;
        let mut second =
            import_image(&path, cache, CacheReuse::Compatible, context.memory(), 11, context)?;
        assert_ne!(first.descriptor().asset(), second.descriptor().asset());
        assert_eq!(pixels(&mut first, context)?[0], Pixel::linear(1.0, 0.0, 0.0, 1.0)?);
        assert_eq!(pixels(&mut second, context)?[0], Pixel::linear(0.0, 0.0, 1.0, 1.0)?);
        Ok(())
    })
}

#[test]
fn corrupt_header_is_an_observable_miss_and_replacement_preserves_the_old_generation()
-> harness::TestResult {
    harness::run_storage(64 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let path = root.path().join("original.png");
        std::fs::write(&path, encoded(&[255, 0, 0, 0, 255, 0])?)?;
        let cache = RuntimeCache::open(
            &root.path().join("cache"),
            CachePolicy { total_bytes: 1024 * 1024 },
            context,
        )?;
        let original = import_image(
            &path,
            cache.clone(),
            CacheReuse::Compatible,
            context.memory(),
            10,
            context,
        )?;
        let descriptor = original.descriptor();
        let key = storage_key(&descriptor.asset().0);
        drop(original);
        let mut bad = cache.clone().fresh(
            Namespace::ImageSources,
            &key,
            descriptor.layout().length(),
            11,
            context,
        )?;
        bad.write("source.raster", b"corrupt", context)?;
        bad.publish(context)?;
        let mut preserved = bad.into_file_reader("source.raster", context)?;
        let mut recovered = import_image(
            &path,
            cache.clone(),
            CacheReuse::Compatible,
            context.memory(),
            12,
            context,
        )?;
        assert_eq!(pixels(&mut recovered, context)?[0], Pixel::linear(1.0, 0.0, 0.0, 1.0)?);
        let mut old = [0; 7];
        preserved.read_exact(&mut old)?;
        assert_eq!(&old, b"corrupt");
        assert_eq!(
            cache.maintain(12 + Namespace::ImageSources.retention(), context)?.pinned_entries,
            2
        );
        Ok(())
    })
}

#[test]
fn lazy_chunk_corruption_requests_explicit_rebuild_from_the_unchanged_original()
-> harness::TestResult {
    harness::run_storage(64 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let path = root.path().join("original.png");
        let encoded = encoded(&[255, 0, 0, 0, 255, 0])?;
        std::fs::write(&path, &encoded)?;
        let cache = RuntimeCache::open(
            &root.path().join("cache"),
            CachePolicy { total_bytes: 1024 * 1024 },
            context,
        )?;
        let original = import_image(
            &path,
            cache.clone(),
            CacheReuse::Compatible,
            context.memory(),
            10,
            context,
        )?;
        let descriptor = original.descriptor();
        drop(original);
        let mut bytes = Vec::new();
        write_raster(&mut bytes, descriptor, context, |_, row| {
            row.fill(255);
            Ok(())
        })?;
        bytes[160] ^= 1;
        let mut bad = cache.clone().fresh(
            Namespace::ImageSources,
            &storage_key(&descriptor.asset().0),
            bytes.len() as u64,
            11,
            context,
        )?;
        bad.write("source.raster", &bytes, context)?;
        bad.publish(context)?;
        drop(bad);
        let mut source = import_image(
            &path,
            cache.clone(),
            CacheReuse::Compatible,
            context.memory(),
            12,
            context,
        )?;
        let failure = pixels(&mut source, context)
            .err()
            .ok_or(ImportError::Invalid("Corrupt chunk was accepted"))?;
        assert!(std::error::Error::source(&failure).is_some());
        drop(source);
        let mut rebuilt =
            import_image(&path, cache, CacheReuse::Rebuild, context.memory(), 13, context)?;
        assert_eq!(pixels(&mut rebuilt, context)?[0], Pixel::linear(1.0, 0.0, 0.0, 1.0)?);
        assert_eq!(std::fs::read(path)?, encoded);
        Ok(())
    })
}
