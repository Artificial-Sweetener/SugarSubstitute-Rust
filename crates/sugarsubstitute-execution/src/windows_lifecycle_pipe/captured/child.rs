// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind inherited capture data to the actual authenticated child and retained kernel capabilities.
use super::codec::CapturedFacts;
use crate::{
    capture_bundle::CaptureBundleRetirement,
    desktop_capture::{
        InheritedCaptureAttachInput,
        input::InheritedCaptureLease,
        native_abi::{CleanupSource, IssueSource, NativeCaptureInput},
    },
    lifecycle_pipe::{AuthenticatedStartupSnapshot, LifecyclePipeFailure, VerifiedPeer, failure},
    native::NativeObservationError,
    native_pipe::security,
    windows_lifecycle_pipe::{native::Lease, native::peer},
};
use sugarsubstitute_contracts::{crash::CaptureTarget, execution::JobContext};
use windows::Win32::{Foundation::*, System::Threading::*};

/// These private facts are constructed only by actual kernel/capability/full-run authentication.
/// The child self observation is retained separately from creator and handler facts.
pub(crate) struct CapturedChildBinding {
    facts: CapturedFacts,
    target: CaptureTarget,
    child_pid: u32,
    child_birth: u64,
}

/// Return the two original loans together, after actual native authentication.
/// Dropping either value only returns its borrow; the lifecycle worker owns physical closure.
pub struct CapturedStartup {
    /// Actual native inherited capture input for the once-only child4MiB job.
    pub capture: InheritedCaptureAttachInput,
    /// Original immutable Launch bytes; consume this batch before creating the UI.
    pub startup: AuthenticatedStartupSnapshot,
}

/// Exactly one native-authenticated purpose-specific input in the existing shared slot.
pub(crate) enum CapturedInput {
    Desktop(CapturedStartup),
    BackendCustody(InheritedCaptureAttachInput),
}
/// Cache-only borrower; no native handle, scheduler, source retirement or renewed budget is owned.
struct InputLease {
    binding: CapturedChildBinding,
    returned: bool,
}
impl InheritedCaptureLease for InputLease {
    fn native_input(
        &self,
        cleanup: CleanupSource,
        issue: IssueSource,
        event: usize,
    ) -> Result<NativeCaptureInput, NativeObservationError> {
        if self.returned || self.binding.child_pid == 0 || self.binding.child_birth == 0 {
            return Err(NativeObservationError::new(
                "captured_input_returned",
                std::io::Error::other("Inherited input borrow returned"),
            ));
        }
        NativeCaptureInput::from_captured(&self.binding, cleanup, issue, event)
    }
    fn return_unissued(&mut self) -> Result<(), NativeObservationError> {
        self.returned = true;
        Ok(())
    }
    fn retire(
        &mut self,
        original: &CaptureBundleRetirement,
    ) -> Result<bool, NativeObservationError> {
        original.check().map_err(|cause| {
            NativeObservationError::new("captured_input_original_cleanup", cause)
        })?;
        // This lease owns only an actual capability borrow. Returning it does not close
        // the separately retained physical section/process owner or assert its retirement.
        self.returned = true;
        Ok(true)
    }
}
impl CapturedChildBinding {
    /// Borrow immutable actual authenticated data only for the private ABI constructor.
    pub(crate) fn facts(&self) -> (&CapturedFacts, CaptureTarget) {
        (&self.facts, self.target)
    }
}

impl Lease {
    /// Read the strict sole v3 section before opening the exact parent lifecycle endpoint.
    /// Original counter conversion reuses the actual QPC owner and performs no renewal.
    pub(in crate::windows_lifecycle_pipe) fn prepare_captured_child(
        &mut self,
        context: &JobContext,
    ) -> Result<(), LifecyclePipeFailure> {
        let original = self.original.ok_or_else(|| failure("captured_client_deadline"))?;
        self.section.accept_input(&mut self.input);
        let (facts, data) = self.section.read_captured(original, self.expected)?;
        self.captured_until = Some(
            crate::counter_deadline::decode_sampled(
                || Ok(context.now()),
                crate::windows_counter::sample,
                facts.source,
            )
            .map_err(LifecyclePipeFailure::native)?,
        );
        self.captured_facts = Some(facts);
        self.data = Some(data);
        Ok(())
    }
    /// Adopt all actual inherited process outputs before validating their native status.
    /// The authenticated parent peer and capability have already been checked by this lease.
    pub(in crate::windows_lifecycle_pipe) fn authenticate_captured_child(
        &mut self,
        parent: VerifiedPeer,
    ) -> Result<(), LifecyclePipeFailure> {
        if self.captured_ready.is_some() {
            return Err(failure("captured_child_replay"));
        }
        let facts = self.captured_facts.as_ref().ok_or_else(|| failure("captured_child_facts"))?;
        if parent.pid != facts.creator_pid || parent.birth != facts.creator_birth {
            return Err(failure("captured_parent_peer_mismatch"));
        }
        for (handle, raw) in self.capture_processes.iter_mut().zip(facts.processes) {
            handle.adopt_partial(HANDLE(raw as *mut _));
        }
        for handle in &self.capture_processes {
            // SAFETY: the dedicated child's inherited process output remains in this exact slot.
            unsafe { SetHandleInformation(handle.raw(), HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0)) }
                .map_err(|cause| {
                super::super::native::error("captured_clear_process_inheritance", cause)
            })?;
        }
        let expected =
            [(facts.creator_pid, facts.creator_birth), (facts.handler_pid, facts.handler_birth)];
        let identity = self.identity.ok_or_else(|| failure("captured_child_identity"))?;
        for (index, wanted) in expected.into_iter().enumerate() {
            let handle = &self.capture_processes[index];
            peer::birth(security::process_facts(handle.raw(), handle)?, wanted)?;
            let actual = security::process_identity(handle.raw(), &mut self.capture_tokens[index])?;
            peer::identity(actual, identity, "captured_process_identity_mismatch")?;
            peer::live(handle.raw())?;
        }
        // SAFETY: the current-process pseudo handle is observed and never adopted or closed.
        let (child_pid, child_birth) =
            security::process_facts(unsafe { GetCurrentProcess() }, &self.own_token)?;
        if expected.iter().any(|(pid, _)| *pid == child_pid) {
            return Err(failure("captured_child_self_alias"));
        }
        let data = self.data.as_ref().ok_or_else(|| failure("captured_child_base"))?;
        let target =
            data.binding.capture_target().ok_or_else(|| failure("captured_child_target"))?;
        let until = self.captured_until.ok_or_else(|| failure("captured_child_original_until"))?;
        // Both loans are installed in this retained native output before eligibility is published.
        let input_loan = self.section.loan(0..super::codec::BASE_BYTES)?;
        let startup_loan = if target.desktop().is_some() {
            Some(self.section.loan(facts.payload.clone())?)
        } else {
            None
        };
        let facts = self.captured_facts.take().ok_or_else(|| failure("captured_child_facts"))?;
        let binding = CapturedChildBinding { facts, target, child_pid, child_birth };
        let capture = InheritedCaptureAttachInput::authenticated(
            target,
            until,
            Box::new(InputLease { binding, returned: false }),
            input_loan,
        );
        self.captured_ready = Some(match startup_loan {
            Some(startup) => CapturedInput::Desktop(CapturedStartup {
                capture,
                startup: AuthenticatedStartupSnapshot::from_authenticated_loan(startup),
            }),
            None => CapturedInput::BackendCustody(capture),
        });
        Ok(())
    }
}

/// Include the actual input lease and public pair in the original map control preflight.
pub(in crate::windows_lifecycle_pipe) const fn storage_bytes() -> usize {
    size_of::<InputLease>() + size_of::<CapturedStartup>()
}
