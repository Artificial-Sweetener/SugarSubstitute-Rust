// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer compositor waits without transferring UI-affine surface ownership.

use super::*;

/// Pair the desktop's completion observation with the worker's untransferred input.
pub(super) struct FencedCommit {
    pub(super) fence: crate::canvas::CompositionFence,
    pub(super) wait: Option<crate::canvas::CompositionWait>,
}

impl FencedCommit {
    /// Bind the observation to this device after a successful commit.
    pub(super) fn new(device: IDCompositionDevice) -> Self {
        let (fence, wait) = crate::canvas::CompositionFence::new(device);
        Self { fence, wait: Some(wait) }
    }

    /// Preserve native resources when an admitted wait did not establish completion.
    pub(super) fn check(&self) -> Result<(), DesktopError> {
        if self.fence.failed() {
            Err(DesktopError::message("composition_wait", "compositor completion failed"))
        } else {
            Ok(())
        }
    }

    /// Accept only actual compositor completion, not a queued commit.
    pub(super) fn completed(&self) -> bool {
        self.fence.completed()
    }
}

impl Composition {
    /// Reap only completed displaced surfaces, then offer one retained native wait.
    fn next_wait(
        &mut self,
    ) -> Result<Option<&mut Option<crate::canvas::CompositionWait>>, DesktopError> {
        if let Some(canvas) = &mut self.canvas {
            canvas.reap_retirements()?;
            if let Some(wait) = canvas.next_wait() {
                return Ok(Some(wait));
            }
        }
        if let Some(fence) = &mut self.retirement_fence {
            fence.check()?;
            if fence.wait.is_some() {
                return Ok(Some(&mut fence.wait));
            }
        }
        Ok(None)
    }
}

impl MainActor {
    /// Register before the actor can create a native composition device.
    pub(crate) fn register_composition(
        &mut self,
        content: &mut impl crate::StartupWindowContent,
    ) -> Result<bool, DesktopError> {
        if !self.composition_registered && !self.admission.closing() {
            let accepted = content
                .main_composition_started(self.identity())
                .map_err(|cause| DesktopError::boxed("register_composition", cause))?;
            self.composition_registered = accepted;
        }
        Ok(self.composition_registered)
    }

    /// Transfer at most one original wait while retaining rejected input on this thread.
    pub(crate) fn progress_composition_wait(
        &mut self,
        content: &mut impl crate::StartupWindowContent,
    ) -> Result<(), DesktopError> {
        let identity = self.identity();
        let Some(composition) = &mut self.composition else { return Ok(()) };
        if let Some(input) = composition.next_wait()? {
            let accepted = content
                .main_composition_wait(identity, input)
                .map_err(|cause| DesktopError::boxed("admit_composition_wait", cause))?;
            if accepted != input.is_none() {
                return Err(DesktopError::message(
                    "admit_composition_wait",
                    "input custody changed unexpectedly",
                ));
            }
        }
        Ok(())
    }
}
