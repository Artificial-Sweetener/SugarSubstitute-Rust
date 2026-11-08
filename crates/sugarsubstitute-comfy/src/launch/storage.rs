// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe explicit storage arguments through the authoritative managed ComfyUI option grammar.
use super::{LaunchConfiguration, ManagedLaunchError, options};

/// A separately selectable ComfyUI storage directory; these selections grant no data ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageDirectory {
    /// Base directory from which ComfyUI derives otherwise unspecified storage paths.
    Base,
    /// User settings and workflow directory.
    User,
    /// Temporary inference artifacts.
    Temporary,
    /// Authored input files.
    Input,
    /// Generated output files.
    Output,
    /// Models selected independently of the backend source.
    Models,
}
impl StorageDirectory {
    fn index(self) -> usize {
        match self {
            Self::Base => 0,
            Self::User => 1,
            Self::Temporary => 2,
            Self::Input => 3,
            Self::Output => 4,
            Self::Models => 5,
        }
    }
}

/// Borrow the last explicit value for each storage option without rewriting original argv.
/// Even an explicitly empty value remains a selection, rather than an installation default.
pub struct ExplicitStorageDirectories<'a> {
    values: [Option<&'a str>; 6],
}
impl<'a> ExplicitStorageDirectories<'a> {
    /// Return original value text, including Unicode/empty values, under ComfyUI's last-value rule.
    pub fn get(&self, directory: StorageDirectory) -> Option<&'a str> {
        self.values[directory.index()]
    }
}

/// Inspect bounded requested argv using the same complete names/arities as managed launch.
/// The result is inert selection data; only verified installation provenance can admit migration.
/// # Errors
/// Rejects protected/unknown options, abbreviations and invalid value counts before any result.
pub fn explicit_storage_directories(
    requested: &LaunchConfiguration,
) -> Result<ExplicitStorageDirectories<'_>, ManagedLaunchError> {
    let mut result = ExplicitStorageDirectories { values: [None; 6] };
    options::visit(requested.arguments(), |name, value| {
        let directory = match name {
            "--base-directory" => StorageDirectory::Base,
            "--user-directory" => StorageDirectory::User,
            "--temp-directory" => StorageDirectory::Temporary,
            "--input-directory" => StorageDirectory::Input,
            "--output-directory" => StorageDirectory::Output,
            "--models-directory" => StorageDirectory::Models,
            _ => return,
        };
        result.values[directory.index()] = value;
    })?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::tests::requested;

    #[test]
    fn original_unicode_empty_equals_and_repeated_values_keep_their_last_explicit_selection()
    -> Result<(), Box<dyn std::error::Error>> {
        let requested = requested(
            [
                "--base-directory=糖 café",
                "--user-directory",
                "",
                "--temp-directory=-1",
                "--output-directory",
                "old",
                "--output-directory=new",
                "--input-directory",
                "画像",
                "--models-directory",
                "模型",
                "--feature-flag=--base-directory=ignored",
            ]
            .map(str::to_owned)
            .into(),
        )?;
        let original = requested.arguments().to_owned();
        let values = explicit_storage_directories(&requested)?;
        for (directory, expected) in [
            (StorageDirectory::Base, "糖 café"),
            (StorageDirectory::User, ""),
            (StorageDirectory::Temporary, "-1"),
            (StorageDirectory::Input, "画像"),
            (StorageDirectory::Output, "new"),
            (StorageDirectory::Models, "模型"),
        ] {
            assert_eq!(values.get(directory), Some(expected));
        }
        assert_eq!(requested.arguments(), original);
        Ok(())
    }

    #[test]
    fn option_values_never_create_storage_selections_and_incomplete_or_short_names_reject()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = requested(vec!["--feature-flag=--user-directory=private".into()])?;
        let values = explicit_storage_directories(&request)?;
        assert_eq!(values.get(StorageDirectory::Base), None);
        assert_eq!(values.get(StorageDirectory::User), None);
        for args in [
            vec!["--base-direct=private"],
            vec!["--user-directory"],
            vec!["--temp-directory", "--cpu"],
        ] {
            let request = requested(args.into_iter().map(str::to_owned).collect())?;
            assert!(explicit_storage_directories(&request).is_err());
        }
        Ok(())
    }
}
