// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual error transfer, first recipient ordering and callback privacy.
use super::*;
use crate::production::child::ProductionChild;
use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_desktop::StartupCauseCode as Code;
struct Opaque {
    callbacks: Arc<AtomicUsize>,
    dropped: Arc<AtomicUsize>,
}
impl fmt::Debug for Opaque {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.callbacks.fetch_add(1, Ordering::SeqCst);
        f.write_str("PRIVATE")
    }
}
impl fmt::Display for Opaque {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.callbacks.fetch_add(1, Ordering::SeqCst);
        f.write_str("PRIVATE")
    }
}
impl Error for Opaque {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.callbacks.fetch_add(1, Ordering::SeqCst);
        None
    }
}
impl Drop for Opaque {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn actual_consume_error_transfer_keeps_first_stage_original_box_and_opaque_callbacks_inert()
-> Result<(), Box<dyn Error>> {
    let callbacks = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicUsize::new(0));
    let original = Box::new(Opaque { callbacks: callbacks.clone(), dropped: dropped.clone() });
    let address = original.as_ref() as *const Opaque;
    let error = observe::<ProductionChild, ()>(StartupConsumeStage::ExecutionDrain, Err(original))
        .err()
        .ok_or("First error absent")?;
    let retained = error.as_ref() as *const dyn Error;
    let error = observe::<ProductionChild, ()>(StartupConsumeStage::EffectRouting, Err(error))
        .err()
        .ok_or("Repeated error absent")?;
    assert!(std::ptr::eq(retained, error.as_ref()));
    let actual = error.downcast_ref::<StartupConsumeFailure>().ok_or("Consumer facts absent")?;
    assert_eq!(actual.stage(), StartupConsumeStage::ExecutionDrain);
    assert_eq!(actual.code(), Code::Opaque);
    assert!(std::ptr::eq(
        address,
        actual.original().downcast_ref::<Opaque>().ok_or("Original payload lost")?
    ));
    assert!(actual.source().is_some());
    assert_eq!(callbacks.load(Ordering::SeqCst), 0);
    assert_eq!(dropped.load(Ordering::SeqCst), 0);
    drop(error);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert_eq!(callbacks.load(Ordering::SeqCst), 0);
    Ok(())
}

#[test]
fn actual_delivery_first_error_projects_render_category_and_keeps_both_original_recipient_boxes()
-> Result<(), Box<dyn Error>> {
    let render = Box::new(std::io::Error::from(std::io::ErrorKind::TimedOut));
    let address = render.as_ref() as *const std::io::Error;
    let error = delivery::<ProductionChild, ()>(
        Err(render),
        Err(Box::new(std::io::Error::from(std::io::ErrorKind::PermissionDenied))),
    )
    .err()
    .ok_or("Combined error absent")?;
    let actual = error.downcast_ref::<StartupConsumeFailure>().ok_or("Consumer facts absent")?;
    assert_eq!(actual.stage(), StartupConsumeStage::RenderDelivery);
    assert_eq!(actual.code(), Code::TimedOut);
    let first = super::super::delivery::first_cause(actual.original())
        .downcast_ref::<std::io::Error>()
        .ok_or("Original render cause lost")?;
    assert!(std::ptr::eq(address, first));
    assert_eq!(
        actual
            .original()
            .source()
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .map(std::io::Error::kind),
        Some(std::io::ErrorKind::TimedOut)
    );
    Ok(())
}

#[test]
fn actual_delivery_success_and_single_child_failure_preserve_effects_and_direct_io_category()
-> Result<(), Box<dyn Error>> {
    assert_eq!(delivery::<ProductionChild, _>(Ok(42), Ok(()))?, 42);
    let original = Box::new(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
    let address = original.as_ref() as *const std::io::Error;
    let error = delivery::<ProductionChild, _>(Ok(42), Err(original))
        .err()
        .ok_or("Actual child error absent")?;
    let actual = error.downcast_ref::<StartupConsumeFailure>().ok_or("Consumer facts absent")?;
    assert_eq!(actual.stage(), StartupConsumeStage::ChildDelivery);
    assert_eq!(actual.code(), Code::Denied);
    assert!(std::ptr::eq(
        address,
        actual.original().downcast_ref::<std::io::Error>().ok_or("Original child issue lost")?
    ));
    for transcript in [
        sugarsubstitute_contracts::transcript::TranscriptError::ZeroIdentity,
        sugarsubstitute_contracts::transcript::TranscriptError::Bound,
        sugarsubstitute_contracts::transcript::TranscriptError::EmptyGap,
        sugarsubstitute_contracts::transcript::TranscriptError::InvalidStyle,
        sugarsubstitute_contracts::transcript::TranscriptError::ForeignResource,
        sugarsubstitute_contracts::transcript::TranscriptError::Sequence,
        sugarsubstitute_contracts::transcript::TranscriptError::StreamEnded,
        sugarsubstitute_contracts::transcript::TranscriptError::Exhausted,
        sugarsubstitute_contracts::transcript::TranscriptError::SnapshotCapacity,
    ] {
        let original = Box::new(crate::production_output::OutputError::Transcript(transcript));
        let address = original.as_ref() as *const crate::production_output::OutputError;
        let error = delivery::<ProductionChild, _>(Ok(42), Err(original))
            .err()
            .ok_or("Transcript child error absent")?;
        let retained =
            error.downcast_ref::<StartupConsumeFailure>().ok_or("Transcript facts absent")?;
        assert_eq!(retained.stage(), StartupConsumeStage::ChildDelivery);
        assert_eq!(retained.code(), Code::Transcript(transcript));
        assert!(std::ptr::eq(
            address,
            retained
                .original()
                .downcast_ref::<crate::production_output::OutputError>()
                .ok_or("Original transcript Box lost")?
        ));
    }
    Ok(())
}

#[test]
fn actual_consume_execution_envelope_projects_native_code_and_preserves_original_box()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::execution::{ExecutionError, Operation};
    use sugarsubstitute_execution::windows::NativeObservationError;
    let native = Box::new(NativeObservationError::new(
        "execution_qpc_counter",
        std::io::Error::from_raw_os_error(233),
    ));
    let native_address = native.as_ref() as *const NativeObservationError;
    let original = Box::new(ExecutionError::new(Operation::NativeLifecycle, native));
    let original_address = original.as_ref() as *const ExecutionError;
    let error = observe::<ProductionChild, ()>(StartupConsumeStage::ExecutionDrain, Err(original))
        .err()
        .ok_or("Actual execution error absent")?;
    let retained = error.downcast_ref::<StartupConsumeFailure>().ok_or("Consumer facts absent")?;
    assert_eq!(retained.stage(), StartupConsumeStage::ExecutionDrain);
    assert_eq!(
        retained.code(),
        Code::Native(sugarsubstitute_desktop::StartupNativeCause::CounterQuery, Some(233))
    );
    assert!(std::ptr::eq(
        original_address,
        retained.original().downcast_ref::<ExecutionError>().ok_or("Original Box lost")?
    ));
    assert!(std::ptr::eq(
        native_address,
        NativeObservationError::from_execution_cause(retained.original())
            .ok_or("Native custody lost")?
    ));
    Ok(())
}
