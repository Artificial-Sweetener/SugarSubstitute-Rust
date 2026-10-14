// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! End only the authenticated helper when original owned cleanup stays uncertain.
use super::source::BackendCustodySource;
use crate::ClaimError;
use windows::Win32::System::{
    JobObjects::IsProcessInJob,
    Threading::{ExitProcess, GetCurrentProcess},
};
impl BackendCustodySource {
    /// End this genuine helper after an original failure or queued cleanup cutoff.
    /// This operation releases no persisted generation and reports no settlement.
    /// It requires the independently adopted helper Job and current self membership;
    /// foreign callers cannot construct this source from IDs, bytes or a public context.
    /// # Errors
    /// Refuses unestablished/settled custody or missing current helper membership.
    pub fn exit_unresolved_helper(&self) -> Result<std::convert::Infallible, ClaimError> {
        let mut access = self.access()?;
        let state = access.state()?;
        if state.settled || !state.established || (!state.failed && !self.memory().cleanup_due()) {
            return Err(ClaimError::invalid("custody_helper_exit_not_due"));
        }
        let job = state
            .containment
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("custody_helper_exit_job"))?
            .raw();
        let mut member = windows::core::BOOL::default();
        // SAFETY: This source owns the actual independently verified self-Job.
        // The pseudo process handle is current-only; initialized output is checked.
        unsafe { IsProcessInJob(GetCurrentProcess(), Some(job), &mut member) }
            .map_err(|cause| ClaimError::new("custody_helper_exit_membership", cause))?;
        if !member.as_bool() {
            return Err(ClaimError::invalid("custody_helper_exit_foreign"));
        }
        tracing::error!(
            operation = "backend_custody_unresolved_exit",
            "Authenticated helper exits with original unresolved custody preserved"
        );
        // SAFETY: Only this current authenticated helper is terminated. No foreign
        // process, persisted record or output is declared retired by this exit.
        unsafe { ExitProcess(1) }
    }
}
