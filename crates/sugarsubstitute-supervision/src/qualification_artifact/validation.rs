// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate the original finite worker, bounded lexical namespace and native PNG extent.
use super::{IMAGE_BYTES, OUTPUT_SLOTS};
use crate::ClaimError;
use std::path::{Component, Path, PathBuf, Prefix};
use sugarsubstitute_contracts::execution::{JobContext, Moment};
pub(super) fn check(context: &JobContext, until: Moment) -> Result<(), ClaimError> {
    if context.cancellation_requested() {
        return Err(ClaimError::invalid("preview_artifact_cancelled"));
    }
    let cutoff = context
        .deadline()
        .filter(|deadline| *deadline <= until)
        .ok_or_else(|| ClaimError::invalid("preview_artifact_original_deadline"))?;
    if context.now() >= cutoff {
        return Err(ClaimError::invalid("preview_artifact_original_deadline"));
    }
    Ok(())
}

pub(super) fn validate(path: &Path, (width, height): (u32, u32)) -> Result<(), ClaimError> {
    use std::os::windows::ffi::OsStrExt;
    if !path.is_absolute()
        || !matches!(path.components().next(), Some(Component::Prefix(p))
        if matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
        || path.as_os_str().encode_wide().count() > 1024
        || path.as_os_str().encode_wide().any(|unit| unit == 0)
        || path
            .components()
            .any(|c| !matches!(c, Component::Prefix(_) | Component::RootDir | Component::Normal(_)))
        || path.components().any(|c| {
            matches!(c, Component::Normal(value)
            if value.encode_wide().any(|unit| unit == u16::from(b':'))
                || value.encode_wide().last().is_some_and(|unit|
                    unit == u16::from(b'.') || unit == u16::from(b' ')))
        })
        || path.extension().is_none_or(|extension| extension != "png")
        || path.components().count() > OUTPUT_SLOTS + 1
        || width == 0
        || height == 0
        || u64::from(width) * u64::from(height) > 8_388_608
    {
        return Err(ClaimError::invalid("preview_artifact_input"));
    }
    Ok(())
}

/// Copy bounded parent bytes fallibly before mutable native ownership changes.
pub(super) fn copy_parent(destination: &Path) -> Result<PathBuf, ClaimError> {
    let parent =
        destination.parent().ok_or_else(|| ClaimError::invalid("preview_artifact_parent"))?;
    let mut copied = PathBuf::new();
    copied
        .try_reserve_exact(parent.as_os_str().len())
        .map_err(|cause| ClaimError::new("allocate_preview_parent", cause))?;
    copied.push(parent);
    if copied.capacity() > 4096 {
        return Err(ClaimError::invalid("preview_artifact_path_capacity"));
    }
    Ok(copied)
}

pub(super) fn validate_png(bytes: &[u8], viewport: (u32, u32)) -> Result<(), ClaimError> {
    if bytes.len() < 33
        || bytes.len() > IMAGE_BYTES
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || &bytes[8..16] != b"\0\0\0\rIHDR"
    {
        return Err(ClaimError::invalid("preview_artifact_png_header"));
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    if (width, height) != viewport {
        return Err(ClaimError::invalid("preview_artifact_png_viewport"));
    }
    Ok(())
}
