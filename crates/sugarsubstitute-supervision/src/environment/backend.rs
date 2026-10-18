// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose the captured managed Comfy process environment without changing its host.
use super::{
    EnvironmentOverride, LaunchEnvironment, LaunchEnvironmentError, MAX_UNITS, MAX_VARIABLES,
    entries,
};
use std::{os::windows::ffi::OsStrExt, path::Path};
use sugarsubstitute_contracts::configuration::LaunchConfiguration;

type Result<T> = std::result::Result<T, LaunchEnvironmentError>;

impl LaunchEnvironment {
    /// Prepare the managed backend's selected Python and captured UTF-8 output.
    ///
    /// The selected interpreter directory precedes the original PATH. The sole
    /// private marker has a fixed name/value and suppresses TTS startup installation.
    /// Explicit `--enable-manager` adds Comfy's root and Manager environment defaults;
    /// an unprobed pygit2 backend is removed. This performs no detection or installation.
    /// Ordinary/private snapshot validation and all native block limits still apply.
    /// # Errors
    /// Preserves malformed snapshot, invalid path, ordinal, allocation and bound causes.
    pub fn for_managed_backend(
        snapshot: &[u16],
        configuration: &LaunchConfiguration,
    ) -> Result<Self> {
        entries::preflight(snapshot, &[])?;
        let initial = entries::merge(snapshot, &[])?;
        let path_name = wide("PATH")?;
        let utf8_name = wide("PYTHONUTF8")?;
        let mut search_path = operational(
            configuration.interpreter().parent().ok_or(LaunchEnvironmentError::InvalidName)?,
        )?;
        let original_path = value(&initial, &path_name)?.unwrap_or(&[]);
        let extra =
            original_path.len().checked_add(1).ok_or(LaunchEnvironmentError::SourceTooLong)?;
        if search_path.len().checked_add(extra).is_none_or(|length| length > MAX_UNITS) {
            return Err(LaunchEnvironmentError::SourceTooLong);
        }
        search_path.try_reserve_exact(extra).map_err(LaunchEnvironmentError::Allocation)?;
        search_path.push(b';' as u16);
        search_path.extend_from_slice(original_path);
        let default_utf8 = value(&initial, &utf8_name)?.is_none();
        drop(initial);

        let encoding_name = wide("PYTHONIOENCODING")?;
        let encoding = wide("utf-8")?;
        let root_name = wide("COMFYUI_PATH")?;
        let root = operational(configuration.root())?;
        let git_name = wide("GIT_PYTHON_REFRESH")?;
        let quiet = wide("quiet")?;
        let pygit_name = wide("CM_USE_PYGIT2")?;
        let enabled = [b'1' as u16];
        let manager =
            configuration.arguments().iter().any(|argument| argument == "--enable-manager");
        let mut changes = Vec::new();
        changes.try_reserve_exact(6).map_err(LaunchEnvironmentError::Allocation)?;
        changes.push(EnvironmentOverride::Set { name: &path_name, value: &search_path });
        changes.push(EnvironmentOverride::Set { name: &encoding_name, value: &encoding });
        if manager {
            changes.push(EnvironmentOverride::Set { name: &root_name, value: &root });
            changes.push(EnvironmentOverride::Set { name: &git_name, value: &quiet });
            changes.push(EnvironmentOverride::Remove { name: &pygit_name });
            if default_utf8 {
                changes.push(EnvironmentOverride::Set { name: &utf8_name, value: &enabled });
            }
        }
        entries::preflight(snapshot, &changes)?;
        let mut merged = entries::merge(snapshot, &changes)?;
        if merged.len() >= MAX_VARIABLES {
            return Err(LaunchEnvironmentError::TooManyVariables);
        }
        let marker = wide("SUGARSUBSTITUTE_SKIP_TTS_INSTALLER")?;
        // Generic overrides cannot insert private names. Only this closed backend
        // constructor adds the exact legacy custom-node semantic, after stripping all
        // private snapshot inputs. No caller-controlled private name/value is accepted.
        let index = match entries::position(&merged, &marker)? {
            Err(index) => index,
            Ok(_) => return Err(LaunchEnvironmentError::PrivateVariable),
        };
        merged.try_reserve_exact(1).map_err(LaunchEnvironmentError::Allocation)?;
        merged.insert(index, entries::Entry { name: &marker, value: Some(&enabled) });
        Self::from_entries(merged)
    }
}

/// Borrow a validated original value using Windows' case-insensitive ordinal rule.
fn value<'a>(entries: &[entries::Entry<'a>], name: &[u16]) -> Result<Option<&'a [u16]>> {
    Ok(match entries::position(entries, name)? {
        Ok(index) => entries[index].value,
        Err(_) => None,
    })
}

/// Preserve exact UTF-16 while making each owned buffer's allocation fallible.
fn wide(text: &str) -> Result<Vec<u16>> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(text.encode_utf16().count())
        .map_err(LaunchEnvironmentError::Allocation)?;
    result.extend(text.encode_utf16());
    Ok(result)
}

/// Reuse the native cwd owner's equivalent operational path spelling.
fn operational(path: &Path) -> Result<Vec<u16>> {
    let mut units = Vec::new();
    units
        .try_reserve_exact(path.as_os_str().encode_wide().count())
        .map_err(LaunchEnvironmentError::Allocation)?;
    units.extend(path.as_os_str().encode_wide());
    crate::command::directory::operational(&units).map_err(LaunchEnvironmentError::BackendPath)
}
