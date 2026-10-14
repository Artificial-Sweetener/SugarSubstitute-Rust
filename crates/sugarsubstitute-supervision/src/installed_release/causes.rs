// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound capture causes independently of diagnostic recovery validation data.
use super::{allocation::*, names::CaptureNameFailure};
use crate::{
    ClaimError,
    claim_error::{BoundedNativeCause, NativeCauseDestination},
};
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::AdmissionError;
pub(crate) enum CaptureBoundedCause {
    Manifest(super::manifest::ManifestError),
    Windows(windows::core::Error),
    RawIo(std::io::Error),
    Lock(std::fs::TryLockError),
    Reserve(std::collections::TryReserveError),
    Integer(std::num::TryFromIntError),
    Unicode(std::string::FromUtf16Error),
    Admission(AdmissionError),
    Name(CaptureNameFailure),
    Status(CaptureCryptoStatus),
}
impl CaptureBoundedCause {
    pub(crate) fn from_native(cause: BoundedNativeCause) -> Result<Self, BoundedNativeCause> {
        Ok(match cause {
            BoundedNativeCause::Windows(c) => Self::Windows(c),
            BoundedNativeCause::RawIo(c) => Self::RawIo(c),
            BoundedNativeCause::Lock(c) => Self::Lock(c),
            BoundedNativeCause::Reserve(c) => Self::Reserve(c),
            BoundedNativeCause::Integer(c) => Self::Integer(c),
            BoundedNativeCause::Unicode(c) => Self::Unicode(c),

            other => return Err(other),
        })
    }
    pub(crate) fn source(&self) -> &(dyn Error + 'static) {
        match self {
            Self::Manifest(c) => c,
            Self::Windows(c) => c,
            Self::RawIo(c) => c,
            Self::Lock(c) => c,
            Self::Reserve(c) => c,
            Self::Integer(c) => c,
            Self::Unicode(c) => c,

            Self::Admission(c) => c,
            Self::Name(c) => c,
            Self::Status(c) => c,
        }
    }
}
#[derive(Debug)]
pub(crate) struct CaptureCryptoStatus(pub(crate) i32);
impl fmt::Display for CaptureCryptoStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CNG status 0x{:08x}", self.0 as u32)
    }
}
impl Error for CaptureCryptoStatus {}
/// Preserve the existing source-owned region before constructing both ticket controls.
pub(crate) struct CaptureCauseDestination {
    pub(crate) allocation: SharedCaptureLoan,
    pub(crate) original: AllocationOrigin,
}
impl AllocationOrigin {
    pub(crate) fn cause_destination(
        &self,
        slot: u8,
    ) -> Result<NativeCauseDestination, CaptureCapacityFailure> {
        let bytes = ClaimError::capture_native_cause_storage_bytes()
            .ok_or(CaptureCapacityFailure::new(CapacityFailureKind::Overflow))?;
        let allocation = SharedCaptureLoan::new(self.reserve(CaptureRegion::Cause(slot), bytes)?)?;
        Ok(NativeCauseDestination::Selected(CaptureCauseDestination {
            allocation,
            original: self.clone(),
        }))
    }
}
