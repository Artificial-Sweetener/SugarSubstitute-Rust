// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain inert local diagnostic storage selection until actual native discovery pins it.
use super::EntryInputError;
use std::path::{Path, PathBuf};
use sugarsubstitute_contracts::configuration::{ConfigurationError, LaunchConfiguration};
/// Select account storage or an explicit private local diagnostic root.
/// Selection grants no filesystem, token or native admission authority.
#[derive(Default)]
pub enum DiagnosticStorageLocation {
    /// Preserve the current primary account's existing protected diagnostic store.
    #[default]
    Account,
    /// Retain an explicit local directory; native discovery validates its actual identity/protection.
    Explicit(PathBuf),
}
impl DiagnosticStorageLocation {
    /// Validate bounded original local path grammar before any native effect.
    /// # Errors
    /// Rejects ambiguous, remote, root-only and excessive retained paths.
    pub fn validate(&self) -> Result<(), EntryInputError> {
        if let Self::Explicit(path) = self {
            LaunchConfiguration::validate_path(path).map_err(EntryInputError::Configuration)?;
            crate::durable_storage::protected::paths::validate_local_shape(path).map_err(|_| {
                EntryInputError::Configuration(ConfigurationError::InvalidLaunchPath)
            })?;
            if path.file_name().is_none() || path.parent().is_none() {
                return Err(EntryInputError::Configuration(ConfigurationError::InvalidLaunchPath));
            }
            if path.capacity() > 256 * 1024 {
                return Err(EntryInputError::Capacity);
            }
        }
        Ok(())
    }
    /// Borrow inert original path selection, without projecting native authority.
    pub fn explicit_path(&self) -> Option<&Path> {
        match self {
            Self::Account => None,
            Self::Explicit(path) => Some(path),
        }
    }
    pub(super) fn capacity(&self) -> usize {
        match self {
            Self::Account => 0,
            Self::Explicit(path) => path.capacity(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_diagnostic_location_rejects_unowned_shape_before_native_effects() {
        for path in [
            "relative",
            r"D:\",
            r"\\server\share\private",
            r"D:\private\..\foreign",
            "D:\\private\\nul\0",
        ] {
            assert!(
                DiagnosticStorageLocation::Explicit(PathBuf::from(path)).validate().is_err(),
                "{path:?}"
            );
        }
        let local = DiagnosticStorageLocation::Explicit(PathBuf::from(r"D:\private-diagnostics"));
        assert!(local.validate().is_ok());
        assert_eq!(local.explicit_path(), Some(Path::new(r"D:\private-diagnostics")));
        assert!(DiagnosticStorageLocation::Account.explicit_path().is_none());
    }
    #[test]
    fn diagnostic_location_retained_capacity_remains_in_original_path_envelope()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut path = PathBuf::from(r"D:\private");
        path.reserve(256 * 1024);
        assert!(matches!(
            DiagnosticStorageLocation::Explicit(path).validate(),
            Err(EntryInputError::Capacity)
        ));
        assert!(size_of::<DiagnosticStorageLocation>() <= 32);
        let mut installation = PathBuf::with_capacity(128 * 1024);
        installation.push(r"E:\installation");
        let mut executable = PathBuf::with_capacity(128 * 1024);
        executable.push(r"E:\installation\host.exe");
        let mut diagnostic = PathBuf::with_capacity(128 * 1024);
        diagnostic.push(r"D:\private");
        let diagnostic = DiagnosticStorageLocation::Explicit(diagnostic);
        diagnostic.validate()?;
        use sugarsubstitute_contracts::{
            configuration::{
                ConnectionConfiguration, LoopbackOrigin, Presentation, ProfileId,
                StartupConfiguration, Theme,
            },
            coordinator_entry::EntryLaunchRequest,
        };
        let configuration = StartupConfiguration::new(
            ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://localhost:8188", 8188)?),
            ProfileId::new("Profile_1".to_owned())?,
            Presentation::new("en", Theme::Light, false)?,
        );
        let input = PathBuf::from(r"E:\inert");
        let request = EntryLaunchRequest::new(
            configuration,
            input.clone(),
            input.clone(),
            input.clone(),
            input.clone(),
            input,
            vec![0, 0],
        )?;
        assert!(matches!(
            super::super::CoordinatorEntryInput::new(installation, executable, diagnostic, request),
            Err(EntryInputError::Capacity)
        ));
        Ok(())
    }
}
