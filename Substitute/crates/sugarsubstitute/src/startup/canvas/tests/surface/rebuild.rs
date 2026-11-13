// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Answer a selection whose cached source is found corrupt while its first frame renders.

use super::*;
use crate::canvas_surface_diagnostic::Stage;
use boundary::Execution;
use viewport::fixture::Fixture;

/// Reconstructed source archives begin with this format identity.
const ARCHIVE_MAGIC: &[u8; 8] = b"SSRAST01";
/// Bytes before the first independently checked source chunk.
const ARCHIVE_HEADER_BYTES: usize = 128;

/// Drain admitted work, including retirement of superseded sources, without host input.
fn quiesce(fixture: &mut Fixture) -> Result<(), Box<dyn Error>> {
    loop {
        let observed = fixture.wake.revision();
        fixture.pump()?;
        assert!(fixture.canvas.take_command().is_none());
        match fixture.execution()? {
            Execution::Idle if fixture.canvas.next_deadline().is_none() => return Ok(()),
            Execution::Idle => fixture.advance(100),
            Execution::Running | Execution::Delivering => fixture.wake.wait_after(observed),
        }
    }
}

/// Invert every chunk byte of the unpinned `width` x `height` archives, keeping each header and
/// length intact so the archive reopens and fails only its chunk integrity checks.
fn corrupt_archives(
    directory: &std::path::Path,
    width: u32,
    height: u32,
) -> Result<usize, Box<dyn Error>> {
    let mut corrupted = 0;
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            corrupted += corrupt_archives(&path, width, height)?;
            continue;
        }
        let mut bytes = std::fs::read(&path)?;
        let matches = bytes.len() > ARCHIVE_HEADER_BYTES
            && bytes.starts_with(ARCHIVE_MAGIC)
            && bytes[8..12] == width.to_le_bytes()
            && bytes[12..16] == height.to_le_bytes();
        if !matches {
            continue;
        }
        for byte in &mut bytes[ARCHIVE_HEADER_BYTES..] {
            *byte = !*byte;
        }
        std::fs::write(&path, &bytes)?;
        corrupted += 1;
    }
    Ok(corrupted)
}

#[test]
fn a_selection_whose_cached_source_is_corrupt_is_applied_from_its_rebuild()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.select(5, "tall.png")?;
    fixture.complete(5)?;
    quiesce(&mut fixture)?;
    let output = fixture.output("wide.png");
    let directory = output
        .parent()
        .and_then(std::path::Path::parent)
        .ok_or("Fixture directory is unavailable")?
        .to_path_buf();
    assert!(corrupt_archives(&directory, 320, 160)? > 0, "the wide archive was not cached");
    let lines = DiagnosticLines::default();
    let capture = lines.observe();
    fixture.select(6, "wide.png")?;
    let replied = fixture.complete(6);
    drop(capture);
    replied?;
    let rebound = lines
        .requests(6)?
        .into_iter()
        .find(|record| record.stage == Stage::Rebound)
        .ok_or("The selection never observed its source rebuild")?;
    assert!(rebound.current > rebound.intent, "{rebound:?}");
    assert_eq!(fixture.accepted()?.extent, sugarsubstitute_image_ops::Extent::new(320, 160)?);
    fixture.finish()
}
