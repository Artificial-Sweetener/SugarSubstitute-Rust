// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Construct bounded child work from the actual fallible Core admission factory.

use super::{
    NativeCaptureDriver,
    control::DesktopCaptureCustody,
    delivery::Delivery,
    input::InheritedCaptureAttachInput,
    metadata::{ChildMetadata, DesktopCaptureCustodyRef, SOURCE_CONTROL_BYTES},
    process_lifetime::{ProcessCaptureCapsule, ProcessCaptureStorage},
    types::DesktopCaptureAttached,
};
use crate::{
    admission::Core,
    capture_attach::{control::Custody, types::CaptureAttachFailure},
    native::Signal,
    scope::Cancellation,
    task::Job,
};
use std::sync::{Arc, atomic::AtomicUsize};
use sugarsubstitute_contracts::execution::{AdmissionError, JobContext, Moment};

pub(super) const CHILD_RESERVED_BYTES: u64 = 1_048_576;

/// Exact child factory input survives central or preallocation rejection unchanged.
pub(super) struct WorkInput {
    pub(super) inherited: InheritedCaptureAttachInput,
    pub(super) driver: Box<dyn NativeCaptureDriver>,
    pub(super) event: super::event::EventFactory,
}

/// Actual worker fields are owned before any external provider/native boundary.
/// The native/input and callback claims are issued only by the actual driver;
/// Core separately issues its4096 Event subdivision and sole2MiB bundle admission.
pub(crate) struct DesktopCaptureWork {
    pub(crate) shared: DesktopCaptureCustodyRef,
    pub(crate) original_until: Moment,
    pub(super) job: Arc<Job<DesktopCaptureAttached, CaptureAttachFailure>>,
    pub(super) inherited: Option<InheritedCaptureAttachInput>,
    pub(super) driver: Option<Box<dyn NativeCaptureDriver>>,
    pub(super) event: super::event::EventFactory,
    pub(super) signal: Arc<Signal>,
    pub(super) delivery: Delivery,
    pub(super) capsule: Option<ProcessCaptureCapsule>,
}

#[repr(C)]
struct JobControlLayout {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: Job<DesktopCaptureAttached, CaptureAttachFailure>,
}

#[repr(C)]
struct CleanupControlLayout {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: Custody,
}

#[repr(C)]
struct SignalControlLayout {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: Signal,
}

impl DesktopCaptureWork {
    /// Count concrete child metadata BEFORE obtaining its claim or allocating boxes.
    /// Actual driver/native input owners have separately retained original claims.
    pub(super) const fn storage_bytes() -> usize {
        size_of::<Self>()
            + size_of::<crate::admission::Record>()
            + size_of::<crate::task::JobHandle<DesktopCaptureAttached, CaptureAttachFailure>>()
            + size_of::<super::submission::DesktopCaptureSession>()
            + size_of::<super::worker::DesktopCaptureActive>()
            + size_of::<ProcessCaptureStorage>()
            + size_of::<super::process_lifetime::ProcessCaptureFields>()
            + size_of::<JobControlLayout>()
            + size_of::<CleanupControlLayout>()
            + size_of::<SignalControlLayout>()
            + DesktopCaptureCustodyRef::storage_bytes()
            + ChildMetadata::storage_bytes()
            + Delivery::storage_bytes()
    }

    /// The same Core factory performs only atomic claims and bounded construction.
    /// No clock, driver, filesystem or native callback is invoked under its guard.
    /// Both checked claims precede ANY child control/Box allocation. On failure
    /// the complete identical input/driver returns to Core's outside-lock rejection.
    pub(super) fn build(
        core: &Arc<Core>,
        input: WorkInput,
        context: &JobContext,
        cancel: Arc<Cancellation>,
        admitted: Moment,
    ) -> Result<Box<Self>, (AdmissionError, WorkInput)> {
        if Self::storage_bytes() > SOURCE_CONTROL_BYTES as usize {
            return Err((AdmissionError::InvalidCaptureAdmission, input));
        }
        let memory = match context.memory().claim(SOURCE_CONTROL_BYTES) {
            Ok(memory) => memory,
            Err(cause) => {
                return Err((AdmissionError::Unexpected(crate::runtime::error(cause)), input));
            }
        };
        let reserved = match context.memory().claim(CHILD_RESERVED_BYTES) {
            Ok(memory) => memory,
            Err(cause) => {
                return Err((AdmissionError::Unexpected(crate::runtime::error(cause)), input));
            }
        };
        let metadata = ChildMetadata::new(memory);
        let cleanup = Custody::new(
            core,
            context.job_id(),
            input.inherited.run(),
            input.inherited.original_deadline(),
        );
        let shared = DesktopCaptureCustody::with_metadata(cleanup, metadata.clone());
        let job =
            Job::new(context.scope_id(), context.spec(), admitted, cancel, Arc::downgrade(core));
        let signal = Signal::new(core);
        let delivery = Delivery::new(metadata.clone());
        let capsule = ProcessCaptureStorage::vacant(metadata, reserved);
        Ok(Box::new(Self {
            shared,
            original_until: input.inherited.original_deadline(),
            job,
            inherited: Some(input.inherited),
            driver: Some(input.driver),
            event: input.event,
            signal,
            delivery,
            capsule: Some(capsule),
        }))
    }
}
