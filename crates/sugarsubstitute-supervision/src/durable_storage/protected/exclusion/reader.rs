// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Acquire only an existing writer fence with the same sealed committed-reader outputs.
use super::*;
impl<const N: usize> HeldFile<N> {
    /// Install the complete counted fence owner before the sole native opener runs.
    /// Only the genuine committed-reader purpose is accepted. OPEN_EXISTING never
    /// creates a missing writer fence, and native acquisition remains exclusive.
    pub(crate) fn acquire_existing_into(
        retained: &mut Option<Self>,
        path: NativeName,
        sid: NativeSid,
        handle: Handle,
        causes: NativeCauseDestinations,
        phase: &mut NativePhase<'_>,
    ) -> Result<(), ClaimError> {
        let budget = handle.budget();
        phase.check_budget(&budget)?;
        let Budget::SelectedRelease(original) = budget else {
            return Err(phase.invalid("committed_reader_exclusion_origin"));
        };
        let same_sid =
            matches!(&sid, NativeSid::Selected(sid) if sid.original().matches(&original));
        if original.original().reader().is_none()
            || retained.is_some()
            || !path.matches_selected(&original)
            || !same_sid
        {
            return Err(phase.invalid("committed_reader_exclusion_identity"));
        }
        causes.matches_selected(&original)?;
        *retained = Some(Self {
            operating: std::sync::atomic::AtomicBool::new(false),
            handle,
            record_path: path.clone(),
            exclusion_path: path,
            locked: false,
            created: false,
            account_sid: sid,
            causes,
            temporary_handles: Mutex::new(Vec::new()),
            stage: Mutex::new(None),
            publication: Mutex::new(None),
            #[cfg(test)]
            fixture_receipt: None,
        });
        let held = retained.as_mut().ok_or_else(|| phase.invalid("missing_committed_fence"))?;
        let result = held.acquire(phase, OPEN_EXISTING, None);
        phase.post_current_for(result, &mut held.causes)
    }
    /// Settle only this actual reader fence under its original paired cleanup proof.
    /// Each fresh unlock/close is checked separately. The result of an accepted
    /// finite unlock is adopted before another check; expiry permits no next call.
    pub(crate) fn retire_reader(
        &mut self,
        proof: &crate::installed_release::SelectedRetirement<'_>,
    ) -> Result<(), ClaimError> {
        let Budget::SelectedRelease(original) = self.handle.budget() else {
            return Err(self.handle.budget().rejection("committed_reader_exclusion_retire"));
        };
        if original.original().reader().is_none() || !proof.matches_origin(original.original()) {
            return Err(ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::identity(),
            ));
        }
        if !lock(&self.temporary_handles).is_empty()
            || lock(&self.stage).is_some()
            || lock(&self.publication).is_some()
        {
            return Err(ClaimError::capture_allocation(
                crate::installed_release::CaptureCapacityFailure::identity(),
            ));
        }
        if self.locked {
            self.causes.check(true)?;
            proof.check().map_err(|cause| self.causes.selected_control(cause, true))?;
            self.handle.borrow_file()?.unlock().map_err(|cause| {
                self.causes.error(
                    "unlock_committed_fence",
                    crate::claim_error::BoundedNativeCause::RawIo(cause),
                    true,
                )
            })?;
            self.locked = false;
        }
        self.handle.close_selected(proof, &mut self.causes)
    }
}
