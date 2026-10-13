// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow callbacks synchronously and retain both distinct outputs before status or unwind.
use super::{
    NativeCaptureDriver,
    issue::{self, Decision},
    metadata::DesktopCaptureCustodyRef,
    native_abi::*,
};
use crate::{
    admission::Core,
    capture_attach::control::{CaptureCleanupSnapshot, CleanupTrigger},
    native::NativeObservationError,
    scope::lock,
    task::Stop,
};
use std::{
    any::Any,
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};
use sugarsubstitute_contracts::execution::JobContext;

type Payload = Box<dyn Any + Send>;
/// Original callback payload and its fatal-clock provenance survive the outer call.
struct StagedPanic {
    payload: Payload,
    fatal_clock: bool,
}
/// Actual initialized slots remain outside every potentially unwinding closure.
/// The pinned native driver continues owning every physical partial output.
#[derive(Default)]
pub(super) struct CallOutputs {
    pub(super) outer: NativeCaptureReply,
    pub(super) nested: NativeCaptureReply,
    pub(super) attach: usize,
    pub(super) foreign_attach: bool,
    pub(super) native_failures: super::native_failures::NativeFailureLedger,
    pub(super) returned_cleanup: Option<ReturnedCleanup>,
}

/// A correlated originally accepted call returned with its exact initialized cell.
/// This marker alone proves neither native closure nor callback quiescence.
pub(super) struct ReturnedCleanup {
    pub(super) attach: usize,
    pub(super) nonce: u64,
}
impl CallOutputs {
    /// Adopt only inert owner tokens; no token is dereferenced or closed here.
    fn adopt(&mut self, token: usize) {
        if token != 0 {
            if self.attach == 0 {
                self.attach = token;
            } else if self.attach != token {
                self.foreign_attach = true;
            }
        }
    }
}
/// Stack-borrowed callback frame is never stored by the exact pinned native ABI.
/// Nested and outer reply storage are separate, including provider/native panic.
struct Frame<'a> {
    core: &'a Core,
    custody: &'a DesktopCaptureCustodyRef,
    context: &'a JobContext,
    driver: &'a dyn NativeCaptureDriver,
    outputs: RefCell<&'a mut CallOutputs>,
    failure: RefCell<Option<NativeObservationError>>,
    panic: RefCell<Option<StagedPanic>>,
    request: RefCell<Option<super::control::AcceptedCaptureStage>>,
}
impl Frame<'_> {
    /// Retain the first callback failure without replacing an earlier panic or native owner.
    fn fail(&self, cause: NativeObservationError) {
        let mut failure = self.failure.borrow_mut();
        if failure.is_none() {
            *failure = Some(cause);
        } else if let Some(first) = &mut *failure {
            first.retain_cleanup(cause);
        }
    }
    /// Catch unwind before the ABI and adopt the nested output before observing it.
    fn issue(&self, attach: usize, stage: u32, nonce: u64) -> u32 {
        if self.panic.borrow().is_some() || self.failure.borrow().is_some() {
            return Decision::Failed as u32;
        }
        if self.request.borrow().is_some() {
            self.fail(NativeObservationError::new(
                "desktop_capture_callback_replay",
                std::io::Error::other("One accepted phase per outer call required"),
            ));
            return Decision::Failed as u32;
        }
        *self.request.borrow_mut() =
            Some(super::control::AcceptedCaptureStage { attach, stage, nonce });
        {
            let mut outputs = self.outputs.borrow_mut();
            outputs.nested = NativeCaptureReply::default();
            outputs.adopt(attach);
        }
        let observed = match self.core.clock.observe_unlatched() {
            Ok(now) => now,
            Err(payload) => {
                self.retain_panic(payload, true);
                return Decision::Failed as u32;
            }
        };
        let mut outputs = self.outputs.borrow_mut();
        let result = catch_unwind(AssertUnwindSafe(|| {
            issue::execute(
                self.core,
                self.custody,
                self.context,
                self.driver,
                super::control::AcceptedCaptureStage { attach, stage, nonce },
                &mut outputs.nested,
                observed,
            )
        }));
        let nested_attach = outputs.nested.attach;
        outputs.adopt(nested_attach);
        match result {
            Ok(Ok(Decision::Accepted)) => match super::native_reply::decode(&outputs.nested) {
                Ok(facts) if !outputs.foreign_attach => {
                    outputs.native_failures.adopt(&facts);
                    if let Some(cause) = facts.primary {
                        self.fail(NativeObservationError::new("desktop_capture_nested", cause));
                        Decision::Failed as u32
                    } else {
                        Decision::Accepted as u32
                    }
                }
                Ok(_) => {
                    self.fail(NativeObservationError::new(
                        "desktop_capture_foreign_output",
                        std::io::Error::other("Native owner token changed"),
                    ));
                    Decision::Failed as u32
                }
                Err(cause) => {
                    self.fail(NativeObservationError::new("desktop_capture_nested_reply", cause));
                    Decision::Failed as u32
                }
            },
            Ok(Ok(decision)) => decision as u32,
            Ok(Err(cause)) => {
                self.fail(cause);
                Decision::Failed as u32
            }
            Err(payload) => {
                self.retain_panic(payload, false);
                Decision::Failed as u32
            }
        }
    }
    /// Retain the first original payload without invoking providers or acquiring owner locks.
    fn retain_panic(&self, payload: Payload, fatal_clock: bool) {
        let mut retained = self.panic.borrow_mut();
        if retained.is_none() {
            *retained = Some(StagedPanic { payload, fatal_clock });
        }
    }
    /// Return the original cleanup tuple; Requested/failure grants no new counter budget.
    fn cleanup(&self, output: &mut CleanupBudget) -> bool {
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.custody.request(Stop::Cancelled, CleanupTrigger::Cancelled)
        }));
        match result {
            Ok(CaptureCleanupSnapshot::Latched { counter: Ok(original), .. }) => {
                *output = CleanupBudget {
                    version: 1,
                    size: 48,
                    state: 1,
                    reserved: 0,
                    started: original.started_counter(),
                    grace: original.graceful_counter(),
                    total: original.total_counter(),
                    frequency: original.frequency(),
                };
                true
            }
            Ok(
                CaptureCleanupSnapshot::Latched { counter: Err(cause), .. }
                | CaptureCleanupSnapshot::ClockUnavailable(cause),
            ) => {
                self.fail(cause);
                false
            }
            Ok(_) => false,
            Err(payload) => {
                self.retain_panic(payload, false);
                false
            }
        }
    }
}
/// SAFETY: the exact native ABI invokes this only within the synchronous call
/// with the unique live Frame pointer; no callback escapes or retains the borrow.
extern "C" fn issue_callback(context: usize, attach: usize, stage: u32, nonce: u64) -> u32 {
    if context == 0 {
        return Decision::Failed as u32;
    }
    // SAFETY: trusted per-call ABI context points to this live, stack-owned Frame.
    let frame = unsafe { &*(context as *const Frame<'_>) };
    match catch_unwind(AssertUnwindSafe(|| frame.issue(attach, stage, nonce))) {
        Ok(decision) => decision,
        Err(payload) => {
            frame.retain_panic(payload, false);
            Decision::Failed as u32
        }
    }
}
/// SAFETY: native caller supplies a complete writable fixed48-byte output only
/// through this synchronous frame; output never aliases either native reply slot.
extern "C" fn cleanup_callback(context: usize, output: *mut CleanupBudget) -> bool {
    if context == 0 || output.is_null() {
        return false;
    }
    // SAFETY: exact trusted ABI provides live distinct frame/output borrows.
    let frame = unsafe { &*(context as *const Frame<'_>) };
    // SAFETY: the exact synchronous ABI gives this distinct complete writable output.
    let output = unsafe { &mut *output };
    *output = CleanupBudget::default();
    match catch_unwind(AssertUnwindSafe(|| frame.cleanup(output))) {
        Ok(result) => result,
        Err(payload) => {
            frame.retain_panic(payload, false);
            false
        }
    }
}
/// Invoke one exact outer call with borrowed sources, then preserve every output
/// before callback/driver failure or the original panic leaves this Rust boundary.
pub(super) fn invoke(
    core: &Core,
    custody: &DesktopCaptureCustodyRef,
    context: &JobContext,
    driver: &dyn NativeCaptureDriver,
    outputs: &mut CallOutputs,
    native: impl FnOnce(
        CleanupSource,
        IssueSource,
        &mut NativeCaptureReply,
    ) -> Result<(), NativeObservationError>,
) -> Result<(), NativeObservationError> {
    let frame = Frame {
        core,
        custody,
        context,
        driver,
        outputs: RefCell::new(outputs),
        failure: RefCell::new(None),
        panic: RefCell::new(None),
        request: RefCell::new(None),
    };
    let address = &frame as *const Frame<'_> as usize;
    let cleanup = CleanupSource { context: address, request: cleanup_callback };
    let issue = IssueSource { context: address, request: issue_callback };
    let mut outer = NativeCaptureReply::default();
    let returned = catch_unwind(AssertUnwindSafe(|| native(cleanup, issue, &mut outer)));
    {
        let mut outputs = frame.outputs.borrow_mut();
        outputs.adopt(outer.attach);
        outputs.outer = outer;
        if let Ok(Err(cause)) = &returned {
            frame.fail(cause.clone());
        }
        match super::native_reply::decode(&outputs.outer) {
            Ok(facts) => outputs.native_failures.adopt(&facts),
            Err(cause) => {
                frame.fail(NativeObservationError::new("desktop_capture_outer_reply", cause))
            }
        }
    }
    // A finite accepted stage has actually returned and its outputs were adopted.
    // Original Core/cleanup/stage order remains passive; no provider/drop crosses it.
    let correlated = {
        let state = lock(&core.state);
        let cleanup = lock(&custody.cleanup.state);
        let mut current = lock(&custody.state);
        let correlated = current.accepted_stage.is_none_or(|accepted| {
            frame.request.borrow().is_some_and(|request| {
                request.attach == accepted.attach
                    && request.stage == accepted.stage
                    && request.nonce == accepted.nonce
                    && current.last_nonce == accepted.nonce
            })
        });
        if correlated {
            current.accepted_stage = None;
        }
        drop(current);
        drop(cleanup);
        drop(state);
        correlated
    };
    if !correlated {
        frame.fail(NativeObservationError::new(
            "desktop_capture_return_correlation",
            std::io::Error::other("Actual accepted phase/call correlation changed"),
        ));
    }
    if let Some(staged) = frame.panic.into_inner() {
        if staged.fatal_clock {
            core.clock.fail_observation(staged.payload);
        }
        resume_unwind(staged.payload);
    }
    if let Err(payload) = returned {
        resume_unwind(payload);
    }
    if let Some(cause) = frame.failure.into_inner() {
        return Err(cause);
    }
    Ok(())
}
