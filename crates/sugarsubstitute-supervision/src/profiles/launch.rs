// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume one profile-bound desktop capsule through the actual registry owner.
use super::*;
use crate::{ClaimLaunchJobs, FamilyBinding, WindowsLaunchInput};
/// Own one exact prepared desktop launch and retain its profile lifetime.
pub struct ProfileDesktopLaunch {
    shared: Arc<Shared>,
    prepared: Arc<AtomicU8>,
    run: RunKey,
    input: Option<WindowsLaunchInput>,
}
impl ProfileDesktopLaunch {
    /// Return peak creation reservation in bytes for the actual prepared input.
    pub fn required_reservation_bytes(&self) -> u64 {
        self.input.as_ref().map_or(0, WindowsLaunchInput::required_reservation_bytes)
    }
}
impl Drop for ProfileDesktopLaunch {
    fn drop(&mut self) {
        let _ = self.prepared.compare_exchange(1, 2, Ordering::AcqRel, Ordering::Acquire);
    }
}
impl WindowsProfileLease {
    /// Prepare one launch while the profile is held; no native effects occur here.
    /// # Errors
    /// Rejects pending, previously attempted or concurrently prepared launches.
    pub fn prepare_desktop_launch(
        &mut self,
        input: WindowsLaunchInput,
    ) -> Result<ProfileDesktopLaunch, ClaimAdmissionError> {
        if self.pending.is_some()
            || self.shared.active.load(Ordering::Acquire) != HELD
            || self.shared.closing.load(Ordering::Acquire)
            || !matches!(self.launch, LaunchState::Unattempted)
            || self.prepared.load(Ordering::Acquire) == 1
        {
            return Err(ClaimAdmissionError::Busy);
        }
        self.prepared.store(1, Ordering::Release);
        Ok(ProfileDesktopLaunch {
            shared: self.shared.clone(),
            prepared: self.prepared.clone(),
            run: self.run,
            input: Some(input),
        })
    }
    /// Submit this exact capsule through its bound registry and execution lifetime.
    /// Rejected registry admission remains latched until any retained watch retires.
    /// # Errors
    /// Rejects a foreign/reused capsule, changed registry/execution or actual registry admission.
    pub fn submit_desktop_launch(
        &mut self,
        execution: &ExecutionClient,
        registry: &mut WindowsProcessRegistry,
        scope: &Scope,
        jobs: ClaimLaunchJobs,
        mut input: ProfileDesktopLaunch,
    ) -> Result<LaunchTicket, ClaimAdmissionError> {
        self.registry_matches(registry)?;
        if self.execution != Some(scope.owner_id())
            || !Arc::ptr_eq(&input.shared, &self.shared)
            || !Arc::ptr_eq(&input.prepared, &self.prepared)
            || input.run != self.run
        {
            return Err(ClaimAdmissionError::Identity);
        }
        if self.pending.is_some()
            || self.shared.active.load(Ordering::Acquire) != HELD
            || self.shared.closing.load(Ordering::Acquire)
            || !matches!(self.launch, LaunchState::Unattempted)
            || self.prepared.load(Ordering::Acquire) != 1
        {
            return Err(ClaimAdmissionError::Busy);
        }
        let native = input.input.take().ok_or(ClaimAdmissionError::Identity)?;
        self.prepared.store(3, Ordering::Release);
        match registry.submit_launch(
            execution,
            scope,
            jobs.native_scope,
            jobs.creation,
            jobs.root_watch,
            FamilyBinding::Desktop(self.run),
            native,
        ) {
            Ok(ticket) => {
                self.launch = LaunchState::Accepted(ticket);
                Ok(ticket)
            }
            Err(cause) => {
                self.launch = LaunchState::Rejected;
                Err(ClaimAdmissionError::Registry(cause))
            }
        }
    }
}
