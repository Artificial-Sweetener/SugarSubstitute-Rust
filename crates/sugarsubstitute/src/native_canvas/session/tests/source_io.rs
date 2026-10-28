// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify first-fallback source-I/O evidence ignores stale and non-preview preparation.

use super::super::completion::retain_first_fallback_io;
use super::*;

mod retained;

#[test]
fn first_fallback_preparation_io_requires_current_preview_and_records_once() {
    let first = ArchiveStats { read_bytes: 10, chunks: 2, range_reads: 3, range_seeks: 1 };
    let later = ArchiveStats { read_bytes: 20, chunks: 4, range_reads: 5, range_seeks: 2 };
    let mut retained = None;

    retain_first_fallback_io(
        &mut retained,
        false,
        BackingTier::Fallback,
        Reconstruction::Preview,
        Some(first),
    );
    retain_first_fallback_io(
        &mut retained,
        true,
        BackingTier::Detail,
        Reconstruction::Preview,
        Some(first),
    );
    retain_first_fallback_io(
        &mut retained,
        true,
        BackingTier::Fallback,
        Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3),
        Some(first),
    );
    assert_eq!(retained, None);

    retain_first_fallback_io(
        &mut retained,
        true,
        BackingTier::Fallback,
        Reconstruction::Preview,
        Some(first),
    );
    retain_first_fallback_io(
        &mut retained,
        true,
        BackingTier::Fallback,
        Reconstruction::Preview,
        Some(later),
    );
    assert_eq!(retained, Some(first));
}
