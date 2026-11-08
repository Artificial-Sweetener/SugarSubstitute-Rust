// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify range bounds and immutable acquisition across actual execution jobs.
use super::*;
mod execution;
fn png() -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&[255, 0, 0, 255])?;
        writer.finish()?;
    }
    Ok(bytes)
}
#[test]
fn acquired_original_reuses_same_verified_handle_for_fresh_head_and_range_jobs()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("retained.png");
    let bytes = png()?;
    std::fs::write(&path, &bytes)?;
    let asset = SessionAsset {
        version: 1,
        digest: Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect(),
        bytes: bytes.len() as u64,
        extension: "png".into(),
        media: "image".into(),
    };
    let registry = Arc::new(Mutex::new(BTreeMap::new()));
    let mut fixture = execution::Fixture::new()?;
    let (readers, expected, selected) = (registry.clone(), asset.clone(), path.clone());
    fixture.run(1_000_000_000, move |context| {
        assert!(!prepared(&readers, &expected)?);
        assert!(read(&readers, &expected, None, true, &context).is_err());
        prepare(&readers, &selected, &expected, context.memory(), &context)?;
        assert!(prepared(&readers, &expected)?);
        Ok(())
    })?;
    assert!(std::fs::write(&path, b"replacement").is_err());
    fixture.expire_acquisition_context();
    let (readers, expected) = (registry.clone(), asset.clone());
    let head = fixture
        .run(3_000_000_000, move |context| read(&readers, &expected, None, true, &context))?;
    assert_eq!(head.total, bytes.len() as u64);
    assert_eq!(head.mime, "image/png");
    assert!(!head.animated && head.bytes.is_empty());
    let (readers, expected) = (registry.clone(), asset.clone());
    let range = fixture.run(3_000_000_000, move |context| {
        read(
            &readers,
            &expected,
            Some(SessionMediaRange::From { start: 1, end: Some(4) }),
            false,
            &context,
        )
    })?;
    assert_eq!(range.bytes, bytes[1..5]);
    let readers = registry.clone();
    fixture.run(3_000_000_000, move |context| {
        let mut readers = readers.lock().map_err(|_| StoreError::Changed)?;
        for reader in readers.values_mut() {
            reader.reader.close(&context)?;
        }
        readers.clear();
        Ok(())
    })?;
    std::fs::write(path, b"replacement after checked retirement")?;
    Ok(())
}
#[test]
fn wrong_original_digest_never_becomes_http_read_authority() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("retained.png");
    let bytes = png()?;
    std::fs::write(&path, &bytes)?;
    let asset = SessionAsset {
        version: 1,
        digest: "f".repeat(64),
        bytes: bytes.len() as u64,
        extension: "png".into(),
        media: "image".into(),
    };
    let mut fixture = execution::Fixture::new()?;
    fixture.run(1_000_000_000, move |context| {
        let readers = Mutex::new(BTreeMap::new());
        assert!(prepare(&readers, &path, &asset, context.memory(), &context).is_err());
        assert!(!prepared(&readers, &asset)?);
        assert!(read(&readers, &asset, None, true, &context).is_err());
        let mut readers = readers.lock().map_err(|_| StoreError::Changed)?;
        for reader in readers.values_mut() {
            reader.reader.close(&context)?;
        }
        Ok(())
    })?;
    Ok(())
}
#[test]
fn large_video_ranges_preserve_total_and_never_exceed_browser_response_admission()
-> Result<(), Box<dyn Error>> {
    let maximum = MAX_MEDIA_BYTES as u64;
    assert_eq!(
        extent(Some(SessionMediaRange::From { start: 0, end: None }), maximum * 4)?,
        (0, maximum)
    );
    assert_eq!(
        extent(Some(SessionMediaRange::From { start: maximum, end: None }), maximum * 4)?,
        (maximum, maximum)
    );
    assert_eq!(
        extent(Some(SessionMediaRange::Suffix { bytes: 16 }), maximum * 4)?,
        (maximum * 4 - 16, 16)
    );
    assert!(extent(None, maximum + 1).is_err());
    assert!(extent(Some(SessionMediaRange::From { start: 4, end: None }), 4).is_err());
    Ok(())
}
