// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stage drag-out copies as real files inside the runtime cache's own staging namespace.
//!
//! Programs that accept only file drops read `<entry>/data/<original filename>` from a fresh
//! entry of the cache's drag-out staging namespace; the user's output folders are never written. A staged copy
//! stays while its pin is held. The namespace keeps no idle age, so cache maintenance and
//! admission remove every copy whose pin was released, including those from earlier runs.

use std::{io, path::PathBuf};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_supervision::cache::{CacheLease, Namespace, RuntimeCache, storage_key};

/// Most recent staged drags kept readable, since a drop target may read after the drag ends.
pub(crate) const STAGED_DRAGS: usize = 4;

/// Longest staged filename leaf, in UTF-16 units, within the cache's 255-unit component rule.
const MAX_NAME_UNITS: usize = 200;

/// One pinned staged copy; dropping it lets later cache maintenance remove the file.
pub(crate) struct StagedDrag {
    _lease: CacheLease,
    /// Absolute path of the staged file, named after the original output.
    pub(crate) path: PathBuf,
}

/// Reduce an output filename to one leaf that every Windows program can open.
///
/// Directory components are dropped, reserved characters become `_`, device names and
/// trailing dots or spaces are neutralised, and an empty result becomes `output`.
pub(crate) fn staged_name(filename: &str) -> String {
    let leaf = filename.rsplit(['/', '\\']).next().unwrap_or_default();
    let mut name = String::new();
    let mut units = 0;
    for character in leaf.chars() {
        let character =
            if character.is_control() || "<>:\"|?*".contains(character) { '_' } else { character };
        units += character.len_utf16();
        if units > MAX_NAME_UNITS {
            break;
        }
        name.push(character);
    }
    let name = name.trim_end_matches(['.', ' ']);
    let stem = name.split('.').next().unwrap_or_default().trim_end().to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|suffix| {
                matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        });
    match name {
        "" | "." | ".." => "output".to_owned(),
        name if device => format!("_{name}"),
        name => name.to_owned(),
    }
}

/// Write `bytes` as a fresh staged file named after `filename` and keep it pinned.
///
/// Call on an execution-owned storage worker. `request` distinguishes this drag's entry.
///
/// # Errors
/// Preserves cache admission, capacity, cancellation and native write failures.
pub(crate) fn stage(
    cache: RuntimeCache,
    request: u64,
    filename: &str,
    bytes: &[u8],
    now: u64,
    context: &JobContext,
) -> io::Result<StagedDrag> {
    let name = staged_name(filename);
    let key = storage_key(format!("drag-out:{request}").as_bytes());
    let mut lease =
        cache.fresh(Namespace::DragOutStaging, &key, bytes.len() as u64, now, context)?;
    lease.write(&name, bytes, context)?;
    lease.publish(context)?;
    let path = lease.path().join(&name);
    Ok(StagedDrag { _lease: lease, path })
}

/// Remove staged copies left unpinned, including those of an earlier run, through maintenance.
///
/// Maintenance verifies each entry's recorded ownership and preserves unknown content. A
/// competing cache writer leaves cleanup to the next admission, which applies the same rule.
pub(crate) fn clean(cache: &RuntimeCache, now: u64, context: &JobContext) {
    if let Err(cause) = cache.maintain(now, context) {
        tracing::warn!(operation = "drag_out_staging_cleanup", cause = %cause,
            "Staged drag-out copies remain until the next cache admission");
    }
}

#[cfg(test)]
mod tests;
