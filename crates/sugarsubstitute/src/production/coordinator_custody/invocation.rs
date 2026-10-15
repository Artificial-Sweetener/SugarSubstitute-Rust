// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate the original witness metadata role before path/environment/native effects.
use super::{
    CoordinatorCustody, LaunchEnvironment, PathBuf, Result, WITNESS_STORAGE_BYTES,
    WindowsLaunchCommand,
};
use std::os::windows::ffi::OsStrExt;
use sugarsubstitute_contracts::lifecycle::LifecycleRejection;

/// Compile-time link to the producer's fixed flag/identity fields array.
pub(super) const FIELDS: usize = 14;

impl CoordinatorCustody {
    /// Bound requested witness layouts from actual path capacities before cloning
    /// them or submitting capture/helper work. Configuration without an environment
    /// reserves this environment owner's maximum unchanged no-overrides-copy bound.
    ///
    /// Includes both retained path copies, actual UTF-16 encoding/growth scratch,
    /// the native command owner's three bounded buffers, original environment copy
    /// and entry scratch, fixed identity/flag argv storage and the boxed controller.
    /// Registry/capture jobs keep their own existing claims. Allocator internals and
    /// compiled stack qualification are separate; this is requested-layout admission.
    /// # Errors
    /// Oversize/overflow rejects with Overloaded before any clone or native effect.
    /// Original environment accounting errors are preserved unchanged.
    pub(crate) fn validate_invocation_storage(
        executable: &PathBuf,
        installation: &PathBuf,
        environment: Option<&LaunchEnvironment>,
    ) -> Result<()> {
        let required = Self::invocation_storage_bytes(executable, installation, environment)?;
        if required > WITNESS_STORAGE_BYTES {
            return Err(Box::new(LifecycleRejection::Overloaded));
        }
        Ok(())
    }

    fn invocation_storage_bytes(
        executable: &PathBuf,
        installation: &PathBuf,
        environment: Option<&LaunchEnvironment>,
    ) -> Result<u64> {
        let overflow = || LifecycleRejection::Overloaded;
        let wide = |path: &PathBuf| {
            let units = path.as_os_str().encode_wide().take(32767).count();
            // The existing Windows command owner includes the terminating NUL.
            if units >= 32767 { Err(LifecycleRejection::Overloaded) } else { Ok(units as u64) }
        };
        let executable_units = wide(executable)?;
        let installation_units = wide(installation)?;
        let paths = (executable.capacity() as u64)
            .checked_add(installation.capacity() as u64)
            .and_then(|bytes| bytes.checked_mul(2))
            .ok_or_else(overflow)?;
        // Installation is encoded both as cwd and the final argv value. Four
        // u16 slots per unit cover current/next Vec growth allocations together.
        let encoded = installation_units
            .checked_mul(2)
            .and_then(|units| units.checked_add(executable_units))
            .and_then(|units| units.checked_mul(4 * size_of::<u16>() as u64))
            .ok_or_else(overflow)?;
        // Actual fixed fields are ASCII flags,64/32-digit namespace/epoch and
        // <=20-digit unsigned native identities.128 per field covers each text
        // and UTF-16 collect capacity/growth;3 vectors cover original+growth argv.
        let arguments = ((FIELDS + 1)
            * 3
            * (size_of::<String>() + size_of::<Vec<u16>>() + 128 + 128 * size_of::<u16>() * 4))
            as u64;
        let environment = match environment {
            Some(environment) => environment.copy_storage_bytes()?,
            None => LaunchEnvironment::maximum_copy_storage_bytes(),
        };
        (size_of::<Self>() as u64)
            .checked_add(paths)
            .and_then(|bytes| bytes.checked_add(encoded))
            .and_then(|bytes| bytes.checked_add(arguments))
            .and_then(|bytes| bytes.checked_add(WindowsLaunchCommand::maximum_storage_bytes()))
            .and_then(|bytes| bytes.checked_add(environment))
            .ok_or_else(|| {
                Box::new(LifecycleRejection::Overloaded) as Box<dyn std::error::Error + Send + Sync>
            })
    }
}
