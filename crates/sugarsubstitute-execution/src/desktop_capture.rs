// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the actual admitted child's attachment, temporary settlement and permanent handoff.

#[allow(unsafe_code)]
mod call;
pub(crate) mod control;
mod delivery;
pub(crate) mod event;
mod failures;
pub(crate) mod input;
mod issue;
pub(crate) mod metadata;
pub(crate) mod native_abi;
mod native_failures;
mod native_reply;
pub(crate) mod process_lifetime;
mod publication;
pub(crate) mod resources;
mod retirement;
mod submission;
mod types;
pub(crate) mod unissued;
pub(crate) mod work;
pub(crate) mod worker;

use crate::{
    capture_bundle::{CaptureBundleAdmission, CaptureBundleRetirement},
    native::NativeObservationError,
};
pub use input::InheritedCaptureAttachInput;
pub use native_abi::{
    NativeCaptureInput, NativeCaptureProgress, NativeCaptureReply, NativeCaptureStep,
};
pub use process_lifetime::ProcessLifetimeCapture;
pub use resources::DESKTOP_CAPTURE_NATIVE_WORKSPACE_BYTES;
pub use submission::{
    DesktopCaptureAdmissionRejection, DesktopCaptureControl, DesktopCaptureSession,
    DesktopCaptureTemporaryState,
};
pub use types::DesktopCaptureAttached;

/// Same-worker adapter retains actual bundle/module/PIN outputs before fallible work.
/// No method creates a worker or retains borrowed per-call native callback sources.
pub trait NativeCaptureDriver: Send {
    /// Consume the sole genuine admitted bundle selector authority on the same worker.
    /// Partial selection/module outputs remain in this already-retained owner.
    fn prepare(&mut self, admission: CaptureBundleAdmission) -> Result<(), NativeObservationError>;
    /// Call the exact pinned ABI with distinct initialized outer output storage.
    fn attach(
        &self,
        input: &NativeCaptureInput,
        output: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError>;
    /// Advance at most one accepted finite stage during this synchronous call.
    fn poll(
        &self,
        input: &NativeCaptureProgress,
        output: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError>;
    /// Accept only the exact bounded C++ stage; finite work follows outside owner locks.
    fn issue_step(
        &self,
        input: &NativeCaptureStep,
        output: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError>;
    /// Adopt exact temporary C++ retirement outputs under the original cleanup tuple.
    fn retire(
        &self,
        input: &NativeCaptureProgress,
        output: &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError>;
    /// Close actual pre-PIN outputs under the outer-held original sealed proof.
    /// Retains all outputs on failure; permanent PIN/source lifetime is not closed here.
    fn retire_prepared(
        &mut self,
        original: &CaptureBundleRetirement,
    ) -> Result<bool, NativeObservationError>;
    /// Observe retained permanent bundle/callback bytes without a provider/native call.
    fn retained_pin_bytes(&self) -> u64;
}

#[cfg(test)]
pub(crate) mod tests;

#[cfg(test)]
mod cancellation_tests;

#[cfg(test)]
mod dispatch_tests;
