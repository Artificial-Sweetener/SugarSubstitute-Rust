// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Capture updater exclusion before local resources exist and bind it to the protected trial.
use super::{disk_inheritance::capture_disk, invalid};
use std::{
    ffi::OsString,
    fs::File,
    io,
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::{
    application_launch::updater::probe::{MAX_PROBE_BYTES, PROBE_ARGUMENT, ProbeRequest},
    configuration::LaunchConfiguration,
};

/// Child-owned updater exclusion; capture alone grants no repository or health authority.
/// Keep this owner through probe execution and execution-owner retirement. Its inheritance bit
/// is cleared at capture, preventing unrelated descendants from retaining updater exclusion.
pub struct InheritedUpdaterProbe {
    pub(super) installation: PathBuf,
    pub(super) request: ProbeRequest,
    pub(super) exclusion: File,
}

impl InheritedUpdaterProbe {
    /// Locally supplied path, subsequently matched against protected installation custody.
    pub fn installation(&self) -> &Path {
        &self.installation
    }
}

/// Adopt exactly one disk handle for the declared probe entry, leaving ordinary entries untouched.
/// Parse all arguments before adoption. No files, diagnostics or execution workers are initialized.
/// # Safety
/// Call once at initial executable entry before local handles or workers exist. The supplied
/// inherited handle must have no other owner, and cannot be closed or reused concurrently.
/// # Errors
/// Rejects unsupported probe versions, malformed arguments and invalid/noninheritable handles.
pub unsafe fn capture_updater_probe(
    arguments: impl IntoIterator<Item = OsString>,
) -> io::Result<Option<InheritedUpdaterProbe>> {
    let Some((installation, request, raw)) = parse(arguments)? else { return Ok(None) };
    // SAFETY: the initial-entry caller supplies the unique ownership and no-reuse contract.
    let exclusion = unsafe { capture_disk(raw) }?;
    Ok(Some(InheritedUpdaterProbe { installation, request, exclusion }))
}

/// Bound every untrusted argument before interpreting a numeric handle as ownership.
fn parse(
    arguments: impl IntoIterator<Item = OsString>,
) -> io::Result<Option<(PathBuf, ProbeRequest, usize)>> {
    let mut arguments = arguments.into_iter();
    let Some(prefix) = arguments.next() else { return Ok(None) };
    if prefix != PROBE_ARGUMENT {
        if prefix.to_str().is_some_and(|value| value.starts_with("--internal-updater-probe-")) {
            return Err(invalid("Unsupported updater probe protocol"));
        }
        return Ok(None);
    }
    let installation =
        PathBuf::from(arguments.next().ok_or_else(|| invalid("Updater installation missing"))?);
    LaunchConfiguration::validate_path(&installation).map_err(io::Error::other)?;
    if installation.capacity() > 256 * 1024 {
        return Err(invalid("Updater installation path capacity exceeded"));
    }
    let encoded = arguments.next().ok_or_else(|| invalid("Updater trial correlation missing"))?;
    if encoded.len() > MAX_PROBE_BYTES {
        return Err(invalid("Oversized updater probe correlation"));
    }
    let encoded =
        encoded.into_string().map_err(|_| invalid("Updater probe correlation is not UTF-8"))?;
    let request = ProbeRequest::decode(encoded.as_bytes()).map_err(io::Error::other)?;
    let spelling = arguments
        .next()
        .and_then(|value| value.into_string().ok())
        .ok_or_else(|| invalid("Updater exclusion handle missing"))?;
    let raw = spelling.parse::<usize>().map_err(|_| invalid("Updater exclusion handle invalid"))?;
    if raw == 0
        || raw >= usize::MAX - 15
        || raw.to_string() != spelling
        || arguments.next().is_some()
    {
        return Err(invalid("Updater probe arguments invalid"));
    }
    Ok(Some((installation, request, raw)))
}

#[cfg(test)]
mod tests;

/// Registry-owned actual terminal decision; never accepted from entry arguments or cached snapshots.
#[derive(Clone, Copy)]
pub(crate) enum UpdaterProbeDisposition {
    Accepted,
    Rejected,
    Unstarted,
}
