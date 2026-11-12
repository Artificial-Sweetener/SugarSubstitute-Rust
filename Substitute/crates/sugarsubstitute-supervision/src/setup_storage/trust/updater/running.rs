// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticate a running accepted engine independently of the process that launched it.
use super::*;
use crate::setup_storage::PreparationImage;

impl TrustStore {
    /// Retain this process's exact accepted updater generation on an admitted storage worker.
    /// The native entry must supply its own `current_exe`, never a command-line image path.
    /// The signed receipt must match this compiled engine release and the current local channel.
    /// Prepared/trial generations cannot qualify, and absent established state is not initialized.
    /// Reserve payload plus trust-operation capacity alongside this store. The returned image
    /// pins every payload file and ancestor without retaining preparation or application locks.
    /// This verifies image identity only: activation must separately reauthorize its candidate.
    /// # Errors
    /// Preserves corrupt continuity and payloads; rejects foreign paths, changed subscriptions,
    /// a different compiled release, cancellation, expired work and native protection failures.
    pub fn retain_running_updater(
        &self,
        executable: &Path,
        context: &JobContext,
    ) -> Result<PreparationImage, TrustFailure> {
        check(context)?;
        self.protected(context, |protection| {
            let _writer = self.writer(protection, context)?;
            let selection = self.updater_locked(protection, SelectionRead::Existing, context)?;
            let generation = selection
                .accepted()
                .ok_or_else(|| invalid("Running updater has no accepted generation"))?;
            let receipt = self.updater_receipt(generation, protection)?;
            if receipt.sequence != sugarsubstitute_update::ENGINE_RELEASE {
                return Err(
                    invalid("Running updater release differs from its accepted receipt").into()
                );
            }
            if receipt.channel != self.subscription_locked(protection, context)?.channel() {
                return Err(invalid("Running updater differs from the current subscription").into());
            }
            let image = self.pin_updater_generation(generation, context)?;
            image.require_executable(executable)?;
            check(context)?;
            Ok(image)
        })
    }
}

#[cfg(test)]
mod tests;
