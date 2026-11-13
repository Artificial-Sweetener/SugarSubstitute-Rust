//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
//! Bound opaque private frames without reproducing the GIMP primitive schema.

use serde::Deserialize;
use std::io::BufRead;

/// Receive the transfer budget compiled from the shared canvas owners.
#[derive(Clone, Copy, Deserialize)]
pub struct NativeLimits {
    /// Bound one frame and all retained outbound bytes.
    #[serde(rename = "frameBytes")]
    pub frame_bytes: usize,
    /// Bound outstanding calls and retained snapshot responses.
    #[serde(rename = "pendingRequests")]
    pub pending_requests: usize,
}

impl NativeLimits {
    /// Read the same packaged limits used by the Python native adapter.
    pub fn packaged() -> Result<Self, String> {
        #[derive(Deserialize)]
        struct Schema {
            #[serde(rename = "x-limits")]
            limits: NativeLimits,
        }
        serde_json::from_str::<Schema>(include_str!("../../../native/native.schema.json"))
            .map(|schema| schema.limits)
            .map_err(|error| format!("Read packaged native limits: {error}"))
    }
}

/// Read one newline frame while checking the budget before growing its allocation.
pub(super) fn read_frame(
    input: &mut impl BufRead,
    maximum: usize,
) -> Result<Option<Vec<u8>>, String> {
    let mut frame = Vec::new();
    loop {
        let available = input
            .fill_buf()
            .map_err(|error| format!("Read native pipe: {error}"))?;
        if available.is_empty() {
            return if frame.is_empty() {
                Ok(None)
            } else {
                Err("The native parent sent an incomplete frame.".into())
            };
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let count = newline.map_or(available.len(), |index| index + 1);
        if count > maximum.saturating_sub(frame.len()) {
            return Err("The native parent exceeded the frame budget.".into());
        }
        frame.extend_from_slice(&available[..count]);
        input.consume(count);
        if newline.is_some() {
            return Ok(Some(frame));
        }
    }
}
