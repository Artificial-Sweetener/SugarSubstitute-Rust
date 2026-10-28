// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A shared original rejects competing imports without holding another worker indefinitely.

use super::*;
use crate::tests::harness;
use sugarsubstitute_supervision::cache::{CachePolicy, Namespace, storage_key};

#[test]
fn concurrent_import_is_a_typed_busy_result_and_the_original_remains_usable() -> harness::TestResult
{
    harness::run_storage(64 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let cache =
            RuntimeCache::open(&root.path().join("cache"), CachePolicy::default(), context)?;
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(png::ColorType::Rgb);
            let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
            writer.write_image_data(&[255, 0, 0]).map_err(std::io::Error::other)?;
        }
        let mut entry = cache.clone().fresh(
            Namespace::ImageSources,
            &storage_key(b"busy encoded original"),
            bytes.len() as u64,
            10,
            context,
        )?;
        entry.write("output.png", &bytes, context)?;
        entry.publish(context)?;
        let mut input = Some(entry.into_file_reader("output.png", context)?);
        let source = ImageSource::encoded(&mut input, context.memory())?;
        let Original::Encoded(encoded) = &source.inner else {
            return Err(ImportError::Invalid("Encoded source lost custody"));
        };
        let held = encoded.reader.lock().map_err(|_| ImportError::Invalid("Test reader failed"))?;
        assert!(matches!(
            source.import_png(cache.clone(), CacheReuse::Compatible, context.memory(), 10, context),
            Err(ImportError::Busy)
        ));
        drop(held);
        let raster =
            source.import_png(cache, CacheReuse::Compatible, context.memory(), 10, context)?;
        assert_eq!(raster.descriptor().layout().extent().width(), 1);
        Ok(())
    })
}
