// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project the supported desktop API onto the one private pipe command owner.
use super::*;
use sugarsubstitute_contracts::execution::OwnedJob;
/// Retain an admitted desktop transport and its actual terminal delivery.
pub struct LifecyclePipeSession {
    /// Fixed typed desktop operations and bounded native observations.
    pub control: LifecyclePipeControl,
    /// Actual I/O/callback/handle settlement, independently of process-family settlement.
    pub completion: OwnedJob<LifecyclePipeReport, LifecyclePipeFailure>,
}
/// Submit desktop transport operations without raw native or scheduler access.
pub struct LifecyclePipeControl {
    pub(crate) inner: Control,
}
impl LifecyclePipeControl {
    /// Transfer the actual authenticated v3 input and original immutable batch once.
    /// Matching metadata and historical observations cannot substitute for this owner.
    #[cfg(windows)]
    pub fn take_captured_startup(
        &self,
    ) -> Option<crate::windows_lifecycle_pipe::captured::child::CapturedStartup> {
        let mut state = lock(&self.inner.shared.state);
        if state.stopped || !state.authenticated {
            return None;
        }
        match state.captured_startup.take()? {
            crate::windows_lifecycle_pipe::captured::child::CapturedInput::Desktop(input) => {
                Some(input)
            }
            other => {
                state.captured_startup = Some(other);
                None
            }
        }
    }
    /// Observe the unchanged original handshake in the execution clock epoch.
    pub fn original_deadline(&self) -> Moment {
        self.inner.original_deadline()
    }
    /// Transfer the original immutable read-only bootstrap pin once.
    pub fn take_bootstrap(&self) -> Result<Option<LifecyclePipeBootstrap>, LifecyclePipeError> {
        self.inner.take_bootstrap()
    }
    /// Observe the exact prepared desktop run; this is not authentication authority.
    pub fn prepared_run(&self) -> Option<RunKey> {
        self.inner.binding().and_then(PeerBinding::desktop)
    }
    /// Take one actual desktop observation without copying a charged frame.
    pub fn take_observation(&self) -> Option<LifecyclePipeObservation> {
        self.inner.take_observation().and_then(Observation::desktop)
    }
    /// Borrow the single preclaimed output guard; occupied storage yields None.
    pub fn take_write_buffer(
        &self,
    ) -> Result<Option<LifecyclePipeWriteBuffer>, LifecyclePipeError> {
        self.inner.take_write_buffer()
    }
    /// Admit one finite read under its unchanged original deadline.
    pub fn read(
        &self,
        operation: LifecyclePipeOperationId,
        original_deadline: Moment,
    ) -> Result<(), LifecyclePipeError> {
        self.inner.read(operation, original_deadline)
    }
    /// Admit an authenticated passive receive with no idle timer.
    pub fn read_passive(
        &self,
        operation: LifecyclePipeOperationId,
    ) -> Result<(), LifecyclePipeError> {
        self.inner.read_passive(operation)
    }
    /// Transfer the exact output guard; rejection returns its original charged allocation.
    pub fn write(
        &self,
        operation: LifecyclePipeOperationId,
        original_deadline: Moment,
        buffer: LifecyclePipeWriteBuffer,
        encoded_length: usize,
    ) -> Result<NativeWriteAdmission, LifecyclePipeWriteRejection> {
        self.inner.write(operation, original_deadline, buffer, encoded_length)
    }
    /// Compare the Hello secret against actual native peer facts before the original handshake expires.
    pub fn verify_capability(
        &self,
        candidate: [u8; 32],
        original_deadline: Moment,
    ) -> Result<(), LifecyclePipeError> {
        self.inner.verify_capability(candidate, original_deadline)
    }
    /// Transfer the validated child's capability once for the native Hello codec.
    pub fn take_client_capability(
        &self,
    ) -> Result<Option<LifecycleClientCapability>, LifecyclePipeError> {
        self.inner.take_client_capability()
    }
    /// Fence native issue and request checked retirement; this creates no cleanup interval.
    pub fn close(&self) {
        self.inner.close()
    }
    pub(crate) fn bind_input(&self, input: PeerInput) {
        self.inner.bind_input(input)
    }
}
/// Preserve a rejected encoded buffer without destroying or overwriting its contents.
pub struct LifecyclePipeWriteRejection {
    /// Typed rejection, without message contents.
    pub error: LifecyclePipeError,
    /// The original owned allocation, available for an explicitly chosen later operation.
    pub buffer: LifecyclePipeWriteBuffer,
}
impl std::fmt::Debug for LifecyclePipeWriteRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LifecyclePipeWriteRejection")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}
/// Pin one immutable read-only bootstrap handle through native launch attributes.
/// Drop only releases the pin and wakes its execution owner; it performs no native close.
pub struct LifecyclePipeBootstrap {
    pub(crate) shared: SharedRef,
}
impl Drop for LifecyclePipeBootstrap {
    fn drop(&mut self) {
        {
            lock(&self.shared.state).pin = false;
        }
        self.shared.changed();
    }
}
impl LifecyclePipeBootstrap {
    pub(crate) fn handle(&self) -> usize {
        lock(&self.shared.state).inherited
    }
}
impl LifecyclePipeBootstrap {
    /// Return immutable deadline facts to pass unchanged to the isolated child role.
    pub fn deadline(&self) -> Option<LifecyclePipeDeadline> {
        lock(&self.shared.state).deadline
    }
}
