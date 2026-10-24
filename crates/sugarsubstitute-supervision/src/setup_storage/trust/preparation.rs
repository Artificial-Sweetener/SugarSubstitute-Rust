// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serialize automatic preparation through native custody independent of application launch.
use super::*;
const PREPARATION: &str = "preparation-v1";
impl TrustStore {
    /// Retain one installation-wide preparation owner across network and staging phases.
    /// The empty protected rendezvous persists; only its live OS lock means busy. It is never
    /// removed, stolen by age/PID, or shared with the application's run or trust-writer locks.
    /// Custody follows this trust store into a prepared candidate and retires with that owner.
    /// Call on the same admitted opening worker before the first repository request.
    /// # Errors
    /// Returns WouldBlock for a live competing preparation. Rejects unknown bytes, unsafe native
    /// ownership and duplicate acquisition; those conditions never authorize cleanup or takeover.
    pub(crate) fn acquire_preparation(&mut self, context: &JobContext) -> Result<(), TrustFailure> {
        if self.preparation.is_some() {
            return Err(invalid("Preparation custody already acquired").into());
        }
        let file = self.protected(context, |protection| {
            let file = protection.rendezvous(&self.root.path.join(PREPARATION), context)?;
            file.try_lock().map_err(io::Error::from)?;
            check(context)?;
            Ok(file)
        })?;
        self.preparation = Some(file);
        Ok(())
    }
}
