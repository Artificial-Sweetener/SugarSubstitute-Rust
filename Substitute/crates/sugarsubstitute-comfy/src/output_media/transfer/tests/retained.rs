// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify independent retained-source cache custody on the real existing storage worker.

use super::*;
use crate::output_media::retain_original;
use sha2::{Digest, Sha256};
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use sugarsubstitute_supervision::cache::utc_seconds;

/// Verify exact publication, unchanged input ownership and rejection before exposure on one admitted worker.
pub(super) fn qualify(
    cache: &RuntimeCache,
    context: &JobContext,
    original: &[u8],
) -> io::Result<()> {
    let expected = OutputArtifactIdentity::new(
        &format!("{:x}", Sha256::digest(original)),
        original.len() as u64,
    )
    .map_err(io::Error::other)?;
    let mut input = Cursor::new(original);
    let mut retained = retain_original(
        &mut input,
        ComfyOutputFormat::Png,
        expected,
        cache.clone(),
        utc_seconds()?,
        context,
    )
    .map_err(io::Error::other)?;
    assert_eq!(input.position(), original.len() as u64);
    let mut bytes = Vec::new();
    retained.read_to_end(&mut bytes)?;
    assert_eq!(bytes, original);
    let mut changed = original.to_vec();
    let last = changed.last_mut().ok_or_else(|| io::Error::other("Empty original fixture"))?;
    *last ^= 1;
    let error = retain_original(
        &mut Cursor::new(changed),
        ComfyOutputFormat::Png,
        expected,
        cache.clone(),
        utc_seconds()?,
        context,
    )
    .err()
    .ok_or_else(|| io::Error::other("Changed retained source was accepted"))?;
    assert_eq!(error.stage, "retained_original_body");
    retained.seek(SeekFrom::Start(0))?;
    let mut preceding = Vec::new();
    retained.read_to_end(&mut preceding)?;
    assert_eq!(preceding, original);
    Ok(())
}
