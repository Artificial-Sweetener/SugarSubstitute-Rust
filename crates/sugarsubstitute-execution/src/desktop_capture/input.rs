// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Move actual authenticated inherited custody into the child worker exactly once.

use super::native_abi::{CleanupSource, IssueSource, NativeCaptureInput};
use crate::{capture_bundle::CaptureBundleRetirement, native::NativeObservationError};
use sugarsubstitute_contracts::{execution::Moment, lifecycle::RunKey};

/// Native-owned actual read-only map/process capabilities, not caller-supplied facts.
/// The concrete native producer validates child self/kernel/peer/capability first.
/// The lease remains outside Core and survives partial closure or callback unwind.
pub(crate) trait InheritedCaptureLease: Send {
    /// Project only already authenticated immutable data; no provider/native call.
    /// Callback values remain borrowed through this ONE synchronous outer invoke.
    fn native_input(
        &self,
        cleanup: CleanupSource,
        issue: IssueSource,
        completion_event: usize,
    ) -> Result<NativeCaptureInput, NativeObservationError>;

    /// Return ONLY an unused borrow to its already retained physical lifecycle owner.
    /// No supplied provider, OS close, counter/deadline renewal or source retirement
    /// may occur here. The physical owner keeps its original map/process/handler
    /// capabilities and cutoff until its own actual checked retirement.
    /// Failure/unwind keeps this same lease in the actual worker for later settlement.
    fn return_unissued(&mut self) -> Result<(), NativeObservationError>;

    /// Close actual temporary inherited capabilities under the original cutoff.
    /// Check before EACH fresh OS call, adopt each returned output before status,
    /// and retain the original owner on partial failure or unwind. Map NoLoan is
    /// only borrow permission; it is never allocation-release proof.
    fn retire(
        &mut self,
        original: &CaptureBundleRetirement,
    ) -> Result<bool, NativeObservationError>;
}

/// One move-only authenticated native input and its original local attach cutoff.
/// Creation is private to the native producer after actual full authentication.
/// Parent startup/body custody and inherited handle ownership are not fabricated
/// from these cache observations; the retained concrete lease owns them.
pub struct InheritedCaptureAttachInput {
    // Deallocate covered input/lease storage before the outside native map charge can return.
    fields: Box<InheritedCaptureFields>,
    #[cfg(windows)]
    _mapping: Option<crate::windows_lifecycle_pipe::StartupViewRetention>,
}

/// Its actual producer preclaims this complete allocation in the original map-control partition.
struct InheritedCaptureFields {
    run: RunKey,
    until: Moment,
    lease: Box<dyn InheritedCaptureLease>,
}

impl InheritedCaptureAttachInput {
    /// Transfer the actual authenticated producer's outside map loan with its native input.
    /// Both covered boxes deallocate before this nonclone loan/charge can return.
    #[cfg(windows)]
    pub(crate) fn authenticated(
        run: RunKey,
        until: Moment,
        lease: Box<dyn InheritedCaptureLease>,
        mapping: crate::windows_lifecycle_pipe::StartupViewLoan,
    ) -> Self {
        Self {
            fields: Box::new(InheritedCaptureFields { run, until, lease }),
            _mapping: Some(mapping.into_retention()),
        }
    }
    /// Count this concrete allocation before the actual native producer allocates it.
    pub(crate) const fn storage_bytes() -> usize {
        size_of::<InheritedCaptureFields>() + size_of::<Self>()
    }
    /// Observe original run cache without invoking the external lease/provider.
    pub fn run(&self) -> RunKey {
        self.fields.run
    }
    /// Observe unchanged local counter-converted cutoff, in nanoseconds.
    pub fn original_deadline(&self) -> Moment {
        self.fields.until
    }
    /// Borrow retained native input only on the actual worker outside all owner locks.
    pub(super) fn lease(&self) -> &dyn InheritedCaptureLease {
        self.fields.lease.as_ref()
    }
    /// Retain the same partial input through each outside-lock finite call/unwind.
    pub(super) fn lease_mut(&mut self) -> &mut dyn InheritedCaptureLease {
        self.fields.lease.as_mut()
    }
    #[cfg(test)]
    /// Fake only the external inherited boundary; actual admission/custody stays unchanged.
    pub(super) fn simulated(
        run: RunKey,
        until: Moment,
        lease: Box<dyn InheritedCaptureLease>,
    ) -> Self {
        Self {
            fields: Box::new(InheritedCaptureFields { run, until, lease }),
            #[cfg(windows)]
            _mapping: None,
        }
    }
    #[cfg(test)]
    /// Replace only the external unused-input boundary before actual admission.
    pub(super) fn replace_lease(&mut self, lease: Box<dyn InheritedCaptureLease>) {
        self.fields.lease = lease;
    }
}
