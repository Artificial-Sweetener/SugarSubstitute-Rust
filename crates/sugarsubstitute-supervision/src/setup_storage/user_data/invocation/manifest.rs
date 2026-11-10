// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode exact passive originals and compare their existing owners' intent to sealed Entry data.
use super::native::Producer;
use crate::durable_storage::invalid;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    io,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};
use sugarsubstitute_configuration::{
    descriptor::LaunchDescriptor,
    paths::StartupPaths,
    startup::{MAX_STARTUP_CONFIGURATION_BYTES, parse_startup_configuration},
};
use sugarsubstitute_contracts::{
    configuration::{ConnectionConfiguration, LaunchConfiguration, StartupConfiguration},
    coordinator_entry::{
        EntryConnectionView, EntryLaunchRequestView, EntryWindowsPathView, MAX_ENTRY_PATH_UNITS,
    },
};

pub(super) const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
pub(super) const MANIFEST: &str = "manifest-v1.json";

#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SourceKind {
    Explicit,
    Descriptor,
}

/// Native source facts are historical provenance only; readers never reopen this path.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Original {
    pub(super) source: Vec<u16>,
    pub(super) original_identity: [u8; 24],
    pub(super) copy_identity: [u8; 24],
    pub(super) bytes: u64,
    pub(super) sha256: [u8; 32],
}
impl Original {
    fn valid(&self) -> bool {
        !self.source.is_empty()
            && self.source.len() <= MAX_ENTRY_PATH_UNITS
            && !self.source.contains(&0)
            && self.original_identity != [0; 24]
            && self.copy_identity != [0; 24]
            && self.bytes <= MAX_STARTUP_CONFIGURATION_BYTES as u64
            && self.sha256 != [0; 32]
    }
}

/// One immutable current invocation, independent of installation-selection or first recovery bytes.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    pub(super) schema_version: u32,
    pub(super) capture: String,
    pub(super) producer: Producer,
    pub(super) directory_identity: [u8; 24],
    pub(super) source_kind: SourceKind,
    pub(super) paths: [Vec<u16>; 6],
    pub(super) startup: Original,
    pub(super) descriptor: Option<Original>,
}
impl Manifest {
    pub(super) fn decode(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(invalid("Invocation manifest exceeds admission"));
        }
        let value: Self = serde_json::from_slice(bytes).map_err(io::Error::other)?;
        if value.schema_version != 1
            || value.capture.len() != 32
            || !value
                .capture
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || !value.capture.bytes().any(|byte| byte != b'0')
            || value.directory_identity == [0; 24]
            || !value.startup.valid()
            || value.descriptor.as_ref().is_some_and(|value| !value.valid())
            || (value.source_kind == SourceKind::Descriptor) != value.descriptor.is_some()
            || value.producer.namespace == [0; 32]
            || value.producer.account_sid.len() > 184
            || value.producer.account_sid.is_empty()
            || value.producer.process == 0
            || value.producer.creation_100ns == 0
            || value.producer.image == [0; 24]
            || value.producer.image_directory == [0; 24]
            || value.producer.root == [0; 24]
        {
            return Err(invalid("Invocation manifest has unsupported or incomplete provenance"));
        }
        // The existing six-path owner preserves exact UTF-16 and its unchanged100KiB capacity.
        value.startup_paths()?;
        Ok(value)
    }
    pub(super) fn startup_paths(&self) -> io::Result<StartupPaths> {
        let mut paths: [PathBuf; 6] = std::array::from_fn(|_| PathBuf::new());
        for (target, units) in paths.iter_mut().zip(&self.paths) {
            if units.is_empty() || units.len() > MAX_ENTRY_PATH_UNITS || units.contains(&0) {
                return Err(invalid("Invocation path exceeds original bounds"));
            }
            *target = PathBuf::from(OsString::from_wide(units));
            target.shrink_to_fit();
        }
        StartupPaths::new(paths).map_err(io::Error::other)
    }
    /// Decode only the retained passive bytes, never later mutable original paths.
    pub(super) fn intent(
        &self,
        startup: &[u8],
        descriptor: Option<&[u8]>,
    ) -> io::Result<(StartupPaths, StartupConfiguration)> {
        if startup.len() > MAX_STARTUP_CONFIGURATION_BYTES
            || descriptor.is_some_and(|value| value.len() > MAX_STARTUP_CONFIGURATION_BYTES)
        {
            return Err(invalid("Invocation original exceeds its document bound"));
        }
        let paths = self.startup_paths()?;
        match (self.source_kind, descriptor) {
            (SourceKind::Explicit, None) => {}
            (SourceKind::Descriptor, Some(bytes)) => {
                let decoded = LaunchDescriptor::parse(bytes)?
                    .into_startup_paths()
                    .map_err(io::Error::other)?;
                if !same_paths(&paths, &decoded) {
                    return Err(invalid("Invocation descriptor disagrees with captured paths"));
                }
            }
            _ => return Err(invalid("Invocation original roles differ from captured source")),
        }
        let configuration = parse_startup_configuration(startup).map_err(io::Error::other)?;
        Ok((paths, configuration))
    }
}
fn same_paths(left: &StartupPaths, right: &StartupPaths) -> bool {
    left.as_paths()
        .iter()
        .zip(right.as_paths())
        .all(|(left, right)| left.as_os_str() == right.as_os_str())
}
fn same_path(left: &Path, right: EntryWindowsPathView<'_>) -> bool {
    left.as_os_str().encode_wide().eq(right.units())
}

/// Compare the decoded original request with the same authenticated sealed intent before migration.
pub(super) fn matches_sealed(
    paths: &StartupPaths,
    configuration: &StartupConfiguration,
    view: EntryLaunchRequestView<'_>,
) -> bool {
    let [_, package, metadata, cubes, script, profile] = paths.as_paths();
    let frontend = view.frontend();
    if !same_path(package, frontend.package_directory())
        || !same_path(metadata, frontend.metadata_file())
        || !same_path(cubes, frontend.cube_directory())
        || !same_path(script, frontend.backend_script())
        || !same_path(profile, view.profile_directory())
        || configuration.profile().as_str() != view.profile()
        || configuration.presentation() != view.presentation()
        || configuration.canvas_renderer() != &view.canvas_renderer().to_owned_preference()
    {
        return false;
    }
    match (configuration.connection(), view.connection()) {
        (ConnectionConfiguration::Attach(left), EntryConnectionView::Attached(right)) => {
            *left == right
        }
        (
            ConnectionConfiguration::Launch(left),
            EntryConnectionView::Managed { root, interpreter, arguments, port },
        ) => {
            same_path(left.root(), root)
                && same_path(left.interpreter(), interpreter)
                && left.port() == port
                && left.arguments().iter().map(String::as_str).eq(arguments.iter())
        }
        _ => false,
    }
}

/// Preserve raw Windows units without using lossy display strings as provenance.
pub(super) fn units(path: &Path) -> io::Result<Vec<u16>> {
    LaunchConfiguration::validate_path(path).map_err(io::Error::other)?;
    let count = path.as_os_str().encode_wide().count();
    let mut value = Vec::new();
    value.try_reserve_exact(count).map_err(io::Error::other)?;
    value.extend(path.as_os_str().encode_wide());
    Ok(value)
}
