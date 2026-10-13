// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind child stages to the actual record and original cleanup custody.
use super::metadata::{ChildMetadata, DesktopCaptureCustodyRef};
use crate::{
    capture_attach::control::{self, CaptureCleanupSnapshot, CleanupTrigger, Custody},
    scope::lock,
    task::Stop,
};
use std::sync::{Arc, Mutex};

/// Native preparation and cleanup phases; none manufactures a native fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DesktopCapturePhase {
    Preparing,
    Attaching,
    Registered,
    Cleaning,
    Retired,
    /// Temporary custody moved; permanent native retention is not retired.
    Transferred,
    /// Unused loan returned; no physical native/source retirement was claimed.
    UnissuedReturned,
}
/// The exact one-shot C++ callback stage accepted on the actual worker.
#[derive(Clone, Copy)]
pub(crate) struct AcceptedCaptureStage {
    pub(crate) attach: usize,
    pub(crate) stage: u32,
    pub(crate) nonce: u64,
}
/// Current stage state remains available while accepted finite work runs unlocked.
pub(crate) struct DesktopCaptureState {
    pub(crate) registered_published: bool,
    pub(crate) phase: DesktopCapturePhase,
    pub(crate) accepted_stage: Option<AcceptedCaptureStage>,
    pub(crate) last_nonce: u64,
    pub(crate) accepted_cleanup: Option<u64>,
    pub(crate) last_cleanup: u64,
    pub(crate) stopped_outcome: Option<Stop>,
}
/// Exact child target; Core stores this allocation and the identical cleanup Arc.
/// Origin and retirement proof stay solely in Active, avoiding a release/Core cycle.
pub(crate) struct DesktopCaptureCustody {
    pub(crate) cleanup: Arc<Custody>,
    pub(crate) state: Mutex<DesktopCaptureState>,
}
impl DesktopCaptureCustody {
    /// Install an already-associated actual cleanup owner before native dispatch.
    pub(crate) fn with_metadata(
        cleanup: Arc<Custody>,
        metadata: ChildMetadata,
    ) -> DesktopCaptureCustodyRef {
        DesktopCaptureCustodyRef::new(
            Self {
                cleanup,
                state: Mutex::new(DesktopCaptureState {
                    registered_published: false,
                    phase: DesktopCapturePhase::Preparing,
                    accepted_stage: None,
                    last_nonce: 0,
                    accepted_cleanup: None,
                    last_cleanup: 0,
                    stopped_outcome: None,
                }),
            },
            metadata,
        )
    }
}

impl DesktopCaptureCustody {
    /// Invalidate fresh effects atomically; sample the existing first cleanup only outside locks.
    /// A held accepted finite stage remains marked until its actual return and adoption.
    pub(crate) fn request(&self, reason: Stop, trigger: CleanupTrigger) -> CaptureCleanupSnapshot {
        if let Some(core) = self.cleanup.core.upgrade() {
            let mut state = lock(&core.state);
            let cleanup = lock(&self.cleanup.state);
            let mut child = lock(&self.state);
            if !matches!(
                child.phase,
                DesktopCapturePhase::Retired
                    | DesktopCapturePhase::Transferred
                    | DesktopCapturePhase::UnissuedReturned
            ) {
                child.phase = DesktopCapturePhase::Cleaning;
                if child.stopped_outcome.is_none() {
                    child.stopped_outcome = Some(reason);
                }
                if let Some(record) = state.jobs.get_mut(&self.cleanup.job)
                    && let Some(deadlines) = &mut record.native_deadlines
                {
                    deadlines.stop();
                }
            }
            drop(child);
            drop(cleanup);
            drop(state);
            core.changed();
        }
        control::request(&self.cleanup, trigger)
    }
}
