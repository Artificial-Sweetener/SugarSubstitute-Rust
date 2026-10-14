// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire actual duplicate pins before releasing current/source storage custody.
use super::*;
impl ProtectedCaptureReadLease {
    /// Checked-close this source lease after the actual native Finished token retires.
    /// Use the same original independently admitted cleanup context, at most15s.
    /// # Errors
    /// Foreign/renewed cleanup or uncertain close retains the entire original owner.
    pub fn retire(&mut self, context: &JobContext) -> Result<(), DiagnosticStorageError> {
        let Some(shared) = &self.shared else {
            return Ok(());
        };
        let mut state = StateAccess::acquire(shared, self.operation);
        state.check(self.operation, context, true)?;
        if state.data_mut()?.finished_owner.is_some() || state.data_mut()?.capture.is_some() {
            return Err(error(self.operation, Code::Busy));
        }
        if state.data_mut()?.capture_reads == 0 {
            return Err(error(self.operation, Code::InvalidIdentity));
        }
        state.data_mut()?.closed = true;
        state.data_mut()?.allocation.close();
        let mut control = DiagnosticNativeRetirement::new(shared.clone(), self.operation, context);
        drop(state);
        self.owner
            .retire(&mut control)
            .map_err(|cause| native(self.operation, Code::Retirement, cause))?;
        let mut state = lock(shared);
        state.capture_reads = state
            .capture_reads
            .checked_sub(1)
            .ok_or_else(|| error(self.operation, Code::InvalidIdentity))?;
        drop(state);
        drop(self.shared.take());
        Ok(())
    }
}
