// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve canonical managed authority while associating its exact output session.

use crate::{FamilyBinding, ManagedLaunchInput, RegistryInputError};
use std::{error::Error, fmt};
use sugarsubstitute_execution::windows_output::{WindowsChildStdio, WindowsOutputControl};

/// Return both unchanged owned inputs when managed output association is rejected.
///
/// The capsule keeps its original canonical claim, preparation state and owner.
/// The unattached stdio remains with its original output session; rejection does
/// not cancel that session or grant access to the capsule's native launch input.
pub struct RejectedManagedOutputLaunch {
    /// Original single-use managed capsule, including previously accepted output.
    pub input: Box<ManagedLaunchInput>,
    /// Original unattached stdio artifact.
    pub stdio: WindowsChildStdio,
    /// Exact association rejection without rendering private launch data.
    pub error: RegistryInputError,
}

impl ManagedLaunchInput {
    /// Associate this capsule with output from its exact resource and generation.
    ///
    /// Binding comes exclusively from this retained capsule. The existing native
    /// launch owner checks output-session provenance and conflicts before transfer.
    /// Accepted association preserves the original canonical claim and preparation
    /// lifetime. Dropping an accepted, unsubmitted capsule requests retirement of
    /// its own output; process settlement and output retirement remain separate.
    ///
    /// # Errors
    /// Returns the original capsule and unattached stdio on resource, provenance,
    /// existing-output or fixture-inheritance conflict. No execution/native effect
    /// occurs and an existing accepted output remains attached to the capsule.
    pub fn with_owned_output(
        mut self,
        control: &WindowsOutputControl,
        stdio: WindowsChildStdio,
    ) -> Result<Self, RejectedManagedOutputLaunch> {
        let Some(input) = self.input.take() else {
            return Err(RejectedManagedOutputLaunch {
                input: Box::new(self),
                stdio,
                error: RegistryInputError::OutputConflict,
            });
        };
        match input.with_owned_output(
            FamilyBinding::Managed(self.resource),
            sugarsubstitute_execution::windows_output::OutputIdentity::Managed(self.resource),
            control,
            stdio,
        ) {
            Ok(input) => {
                self.input = Some(input);
                Ok(self)
            }
            Err(rejected) => {
                self.input = Some(*rejected.input);
                Err(RejectedManagedOutputLaunch {
                    input: Box::new(self),
                    stdio: rejected.stdio,
                    error: rejected.error,
                })
            }
        }
    }
}

impl fmt::Debug for RejectedManagedOutputLaunch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RejectedManagedOutputLaunch")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for RejectedManagedOutputLaunch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl Error for RejectedManagedOutputLaunch {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}
