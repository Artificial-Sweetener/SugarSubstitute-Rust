// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the single original native write token and returned allocation through checked retirement.
use super::*;
use observations::ObservationError;
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    LifecyclePipeWriteBuffer, NativeWriteAdmission,
};
/// Actual native admission owns its correlation and any returned original allocation together.
pub(super) struct PendingWrite {
    receipt: WriteReceipt,
    admission: NativeWriteAdmission,
    header: WireHeader,
    original: Moment,
    returned_verified: bool,
    revoked_before_issue: bool,
    returned: Option<LifecyclePipeWriteBuffer>,
}
impl PendingWrite {
    pub(super) fn new(
        operation: LifecyclePipeOperationId,
        bytes: usize,
        header: WireHeader,
        original: Moment,
        admission: NativeWriteAdmission,
    ) -> Self {
        Self {
            receipt: WriteReceipt::new(operation, bytes),
            admission,
            header,
            original,
            returned_verified: false,
            revoked_before_issue: false,
            returned: None,
        }
    }
    pub(super) const fn header(&self) -> WireHeader {
        self.header
    }
    /// Request only this same admitted write's no-issue veto; issued or uncertain I/O stays live.
    pub(super) fn request_retirement(&mut self) {
        self.revoked_before_issue |= WriteReceipt::permits_no_issue(self.admission.revoke());
    }
    /// Observe genuine issuance; original byte/order checks remain with their passive owner.
    pub(super) fn issued(
        pending: Option<&mut Self>,
        operation: LifecyclePipeOperationId,
    ) -> std::result::Result<(), ObservationError> {
        WriteReceipt::issued(pending.map(|owner| &mut owner.receipt), operation)
    }
    /// Observe full native completion; dropping the original ticket never manufactures it.
    pub(super) fn written(
        pending: Option<&Self>,
        operation: LifecyclePipeOperationId,
        bytes: u32,
    ) -> std::result::Result<(), ObservationError> {
        WriteReceipt::written(pending.map(|owner| &owner.receipt), operation, bytes)
    }
    /// Adopt the actual returned buffer before checking exact identity/order/header.
    /// Failure leaves its allocation and native admission with this original owner.
    pub(super) fn revoked(
        &mut self,
        operation: LifecyclePipeOperationId,
        buffer: LifecyclePipeWriteBuffer,
    ) -> Result<()> {
        if self.returned.is_some() {
            return Err("Original write return repeated".into());
        }
        self.returned = Some(buffer);
        if !self.revoked_before_issue {
            return Err("Native write revocation was not requested by retirement".into());
        }
        self.receipt.revoked(operation)?;
        let bytes =
            self.returned.as_mut().ok_or("Original returned write buffer absent")?.as_mut_bytes();
        let exact =
            bytes.get(..self.receipt.bytes()).ok_or("Original encoded write extent differs")?;
        if FrameCodec::decode(exact)?.header != self.header {
            return Err("Original revoked write header differs".into());
        }
        self.returned_verified = true;
        Ok(())
    }
}

/// Genuine original queued write after its exact returned buffer and callback quiescence.
/// This retains the original opaque native identity; it cannot be copied or constructed from headers.
pub(in crate::production::child) struct UnissuedAcquisition {
    pub(in crate::production::child) header: WireHeader,
    pub(in crate::production::child) original: Moment,
    pub(in crate::production::child) identity: sugarsubstitute_contracts::startup::MainIdentity,
    _admission: NativeWriteAdmission,
}
impl PendingWrite {
    pub(super) fn returned_acquisition(
        &mut self,
    ) -> Result<Option<sugarsubstitute_contracts::startup::MainIdentity>> {
        if !self.returned_verified {
            return Ok(None);
        }
        let bytes =
            self.returned.as_mut().ok_or("Verified original native return absent")?.as_mut_bytes();
        let decoded = FrameCodec::decode(
            bytes.get(..self.receipt.bytes()).ok_or("Original encoded extent differs")?,
        )?;
        match decoded.payload {
            sugarsubstitute_supervision::lifecycle_protocol::DecodedPayload::Acquire {
                identity,
                ..
            } => Ok(Some(identity)),
            _ => Ok(None),
        }
    }
    pub(super) fn returned(&self) -> bool {
        self.returned_verified && self.returned.is_some()
    }
    pub(super) fn into_unissued(
        self,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
    ) -> UnissuedAcquisition {
        UnissuedAcquisition {
            header: self.header,
            original: self.original,
            identity,
            _admission: self.admission,
        }
    }
}
