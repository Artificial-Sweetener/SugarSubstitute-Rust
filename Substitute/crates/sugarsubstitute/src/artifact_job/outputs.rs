// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Supply receipt schemas and retained pixels to the artifact owner after native teardown.
use super::*;
use std::path::PathBuf;
use sugarsubstitute_contracts::presentation::PreparedFrame;
use sugarsubstitute_supervision::{ArtifactPublication, write_artifact};
const RECEIPT_BYTES: usize = 4 * 1024 * 1024;

pub(crate) fn write_json(path: PathBuf, value: serde_json::Value, create: bool) -> Result {
    run(Kind::Record, move |context| {
        write_artifact(
            &path,
            create,
            ArtifactPublication::ReplaceSelected,
            RECEIPT_BYTES,
            &context,
            |output| serde_json::to_writer_pretty(output, &value),
        )
        .map_err(WorkError::new)
    })
}
#[cfg(all(windows, feature = "windows-native-host"))]
pub(crate) fn write_bytes(path: PathBuf, bytes: Vec<u8>, create: bool) -> Result {
    run(Kind::Record, move |context| {
        write_artifact(
            &path,
            create,
            ArtifactPublication::ReplaceSelected,
            RECEIPT_BYTES,
            &context,
            |output| output.write_all(&bytes),
        )
        .map_err(WorkError::new)
    })
}
pub(crate) fn write_frame(path: PathBuf, frame: PreparedFrame) -> Result<PreparedFrame> {
    run(Kind::Record, move |context| {
        write_artifact(
            &path,
            true,
            ArtifactPublication::ReplaceSelected,
            frame.pixels().len(),
            &context,
            |output| output.write_all(frame.pixels()),
        )
        .map_err(WorkError::new)?;
        Ok(frame)
    })
}
