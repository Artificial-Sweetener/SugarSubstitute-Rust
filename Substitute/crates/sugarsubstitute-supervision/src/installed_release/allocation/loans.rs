// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Free exact ticket controls before returning their source-owned storage regions.

use super::*;
use std::sync::{Arc, atomic::AtomicUsize};

/// Retain one preadmitted region outside its concrete data allocation.
pub(crate) struct CaptureLoan {
    pub(super) region: CaptureRegion,
    pub(super) bytes: usize,
    pub(super) original: Option<AllocationOrigin>,
}
impl Drop for CaptureLoan {
    fn drop(&mut self) {
        if let Some(original) = self.original.take()
            && !original.release(self.region, self.bytes)
        {
            std::mem::forget(original);
            tracing::error!(
                operation = "capture_allocation_retained",
                "Capture storage custody retained"
            );
        }
    }
}
struct TicketData {
    region: CaptureRegion,
    bytes: usize,
}
/// Share the existing ticket, keeping its original allocation outside every final control free.
pub(crate) struct SharedCaptureLoan {
    data: Option<Arc<TicketData>>,
    original: AllocationOrigin,
}
#[repr(C)]
struct TicketControl {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: TicketData,
}
impl SharedCaptureLoan {
    pub(crate) const fn control_bytes() -> usize {
        size_of::<TicketControl>()
    }
    pub(crate) fn new(mut loan: CaptureLoan) -> Result<Self, CaptureCapacityFailure> {
        let region = loan.region;
        let bytes = loan.bytes;
        let original = loan
            .original
            .take()
            .ok_or(CaptureCapacityFailure::new(CapacityFailureKind::Missing))?;
        Ok(Self { data: Some(Arc::new(TicketData { region, bytes })), original })
    }
    pub(crate) fn original(&self) -> &AllocationOrigin {
        &self.original
    }
}
impl Clone for SharedCaptureLoan {
    fn clone(&self) -> Self {
        Self { data: self.data.clone(), original: self.original.clone() }
    }
}
impl Drop for SharedCaptureLoan {
    fn drop(&mut self) {
        if let Some(data) = self.data.take()
            && let Some(data) = Arc::into_inner(data)
            && !self.original.release(data.region, data.bytes)
        {
            std::mem::forget(self.original.clone());
            tracing::error!(
                operation = "capture_ticket_retained",
                "Capture storage custody retained"
            );
        }
    }
}
