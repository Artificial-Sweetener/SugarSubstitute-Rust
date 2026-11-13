// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own closed lifecycle transport sessions and fixed command slots after real admission.
mod admission;
pub(crate) mod metadata;
mod shared;
pub(crate) use shared::SharedRef;
mod binding;
mod commands;
mod owner;
mod retirement;
pub(crate) use binding::{Observation, PeerBinding, VerifiedPeer};
pub(crate) use commands::{Control, retain_failure};
pub(crate) use owner::Owner;
pub use retirement::LifecyclePipeRetirement;
mod buffers;
mod control;
pub(crate) mod current_admission;
pub(crate) use current_admission::validate as validate_current;
pub(crate) use current_admission::validate_source as validate_current_source;
pub(crate) use current_admission::validated as with_current_session;
pub(crate) mod enrollment;
mod issue;
#[cfg(test)]
pub(crate) mod tests;
mod types;
pub(crate) mod worker;
use crate::{
    admission::{Core, RequestKind},
    native::{NativeObservationError, Signal},
    native_deadlines::{Deadlines, Role},
    scope::lock,
    task::{Job, JobHandle, QueuedJob, Work},
};
pub use admission::{NativeWriteAdmission, NativeWriteRevocation};
pub(crate) use buffers::CAPTURED_METADATA_BYTES;
pub use buffers::{
    LifecyclePipeWriteBuffer, MIN_CAPTURED_LIFECYCLE_PIPE_BYTES, ReceivedLifecycleFrame,
};
pub use control::{
    LifecyclePipeBootstrap, LifecyclePipeControl, LifecyclePipeSession, LifecyclePipeWriteRejection,
};
pub(crate) use issue::{Activity, Dispatch, Issue};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, Weak},
};
use sugarsubstitute_contracts::{
    execution::{AdmissionError, JobContext, JobSpec, Moment, OwnedJob, Scope},
    lifecycle::{ProcessKey, RunKey},
};
pub use types::*;
// The snapshot borrows the Windows startup mapping loan.
#[cfg(windows)]
mod startup_snapshot;
#[cfg(windows)]
pub use startup_snapshot::AuthenticatedStartupSnapshot;

pub(crate) use crate::native_pipe::Progress;
pub(crate) enum PeerInput {
    #[cfg(windows)]
    Owned { handle: std::os::windows::io::OwnedHandle, process: Option<ProcessKey> },
    #[cfg(windows)]
    Inherited { section: usize },
    #[cfg(windows)]
    RecoveryInherited { section: usize, desktop_section: usize },
    #[cfg(test)]
    Simulated,
}
pub(crate) mod captured_admission;

pub(crate) trait PipeLease: Send {
    fn adopt(&mut self, input: PeerInput) -> Result<(), (Box<LifecyclePipeFailure>, PeerInput)>;
    fn prepare(
        &mut self,
        signals: [Arc<Signal>; 3],
        context: &JobContext,
        handshake: Moment,
    ) -> Result<usize, LifecyclePipeFailure>;
    fn binding(&self) -> Option<PeerBinding>;
    fn deadline(&self) -> Option<LifecyclePipeDeadline>;
    fn capability(&self) -> Option<[u8; 32]>;
    fn connect(&mut self, issue: Issue<'_>) -> Result<Dispatch<Progress>, LifecyclePipeFailure>;
    fn read(
        &mut self,
        bytes: &mut [u8],
        issue: Issue<'_>,
    ) -> Result<Dispatch<Progress>, LifecyclePipeFailure>;
    fn write(
        &mut self,
        bytes: &[u8],
        issue: Issue<'_>,
    ) -> Result<Dispatch<Progress>, LifecyclePipeFailure>;
    fn poll(&mut self, role: Role) -> Result<Progress, LifecyclePipeFailure>;
    fn quiesce(&mut self, role: Role) -> Result<(), LifecyclePipeFailure>;
    fn verify_message(&mut self) -> Result<(), LifecyclePipeFailure>;
    fn authenticate(&mut self, candidate: &[u8; 32]) -> Result<VerifiedPeer, LifecyclePipeFailure>;
    /// Transfer only already adopted actual v3 outputs; external adapters have none by default.
    #[cfg(windows)]
    fn inherited_capture_processes(&self) -> Option<[usize; 2]> {
        None
    }
    /// Transfer already adopted paired output custody after actual native authentication.
    #[cfg(windows)]
    fn take_captured_startup(
        &mut self,
    ) -> Option<crate::windows_lifecycle_pipe::captured::child::CapturedInput> {
        None
    }
    /// Actual paired Root section loan after independent kernel/capability authentication.
    fn take_recovery_desktop_bootstrap(
        &mut self,
    ) -> Option<crate::recovery_supervisor::root_desktop::RootDesktopBootstrapLoan> {
        None
    }
    fn take_completed_write(&mut self) -> Option<u32>;
    fn retire(&mut self, launch_pinned: bool) -> Result<bool, LifecyclePipeFailure>;
}
pub(crate) struct Command {
    pub(crate) operation: LifecyclePipeOperationId,
    /// Only authenticated custody heartbeats use their independent active-time watchdog.
    pub(crate) deadline: Option<Moment>,
}
/// Preserve finite command facts or an authenticated passive receive with no idle timer.
#[derive(Clone, Copy)]
pub(crate) enum ReadDeadline {
    Original(Moment),
    Passive,
}
/// Bind one monotonic read identity to the policy admitted by its session control.
#[derive(Clone, Copy)]
pub(crate) struct ReadCommand {
    pub(crate) operation: LifecyclePipeOperationId,
    pub(crate) deadline: ReadDeadline,
}
pub(crate) struct Write {
    pub(crate) command: Command,
    pub(crate) buffer: buffers::Buffer,
    pub(crate) length: usize,
}
pub(crate) struct Authentication {
    pub(crate) candidate: [u8; 32],
    pub(crate) deadline: Moment,
}
impl Drop for Authentication {
    fn drop(&mut self) {
        self.candidate.fill(0);
    }
}
pub(crate) struct State {
    pub(crate) custody_worker_permit_taken: bool,
    pub(crate) recovery_startup_taken: bool,
    pub(crate) recovery_desktop_bootstrap:
        Option<crate::recovery_supervisor::root_desktop::RootDesktopBootstrapLoan>,
    pub(crate) custody_parent_storage_taken: bool,
    checked_retirement: Option<retirement::CheckedRetirement>,
    #[cfg(windows)]
    pub(crate) capture_inherited: Option<[usize; 2]>,
    #[cfg(windows)]
    pub(crate) captured_startup:
        Option<crate::windows_lifecycle_pipe::captured::child::CapturedInput>,
    pub(crate) stopped: bool,
    pub(crate) prepared: bool,
    pub(crate) connected: bool,
    pub(crate) authenticated: bool,
    pub(crate) authenticated_peer: Option<VerifiedPeer>,
    pub(crate) auth_busy: bool,
    pub(crate) binding: Option<PeerBinding>,
    pub(crate) peer: Option<PeerInput>,
    pub(crate) enrolled: bool,
    pub(crate) enrollment_taken: bool,
    pub(crate) inherited: usize,
    pub(crate) pin: bool,
    pub(crate) transferred: bool,
    pub(crate) input_pin: bool,
    pub(crate) deadline: Option<LifecyclePipeDeadline>,
    pub(crate) capability: Option<[u8; 32]>,
    pub(crate) latest: u64,
    pub(crate) read: Option<ReadCommand>,
    pub(crate) write: Option<Write>,
    pub(crate) authenticate: Option<Authentication>,
    pub(crate) read_busy: bool,
    pub(crate) write_busy: bool,
    pub(crate) write_slots: usize,
    pub(crate) write_admission: Option<admission::Admission>,
    pub(crate) loans: u8,
    /// Native work and every pool buffer retired; later guard drops free their storage.
    pub(crate) retired: bool,
    /// Whether outstanding buffer loans hold back retirement. Owners that order their work by
    /// holding a loan keep this; a purpose whose callers must never be able to stall
    /// retirement clears it at reservation, before any loan exists.
    pub(crate) loans_fence_retirement: bool,
    pub(crate) read_buffer: Option<buffers::Buffer>,
    pub(crate) write_buffer: Option<buffers::Buffer>,
    pub(crate) observations: VecDeque<Observation>,
    pub(crate) received: u64,
    pub(crate) written: u64,
}
impl State {
    fn new(binding: Option<PeerBinding>) -> Self {
        Self {
            custody_worker_permit_taken: false,
            recovery_startup_taken: false,
            recovery_desktop_bootstrap: None,
            custody_parent_storage_taken: false,
            checked_retirement: None,
            #[cfg(windows)]
            capture_inherited: None,
            #[cfg(windows)]
            captured_startup: None,
            stopped: false,
            prepared: false,
            connected: false,
            authenticated: false,
            authenticated_peer: None,
            auth_busy: false,
            binding,
            peer: None,
            enrolled: false,
            enrollment_taken: false,
            inherited: 0,
            pin: false,
            transferred: false,
            input_pin: binding.is_none(),
            deadline: None,
            capability: None,
            latest: 0,
            read: None,
            write: None,
            authenticate: None,
            read_busy: false,
            write_busy: false,
            write_slots: 0,
            write_admission: None,
            loans: 0,
            retired: false,
            loans_fence_retirement: true,
            read_buffer: None,
            write_buffer: None,
            observations: VecDeque::with_capacity(8),
            received: 0,
            written: 0,
        }
    }
    pub(crate) fn room(&self, additional: usize) -> bool {
        self.observations.len()
            + 2 * usize::from(self.read_busy)
            + self.write_slots
            + usize::from(self.auth_busy)
            + additional
            <= 8
    }
}
pub(crate) struct Shared {
    pub(crate) recovery_grant: Option<crate::RegisteredRecoveryGrant>,
    pub(crate) child: bool,
    pub(crate) captured: bool,
    pub(crate) original_deadline: Moment,
    pub(crate) state: Mutex<State>,
    pub(crate) core: Weak<Core>,
    pub(crate) id: sugarsubstitute_contracts::execution::JobId,
}
impl Shared {
    pub(crate) fn changed(&self) {
        if let Some(core) = self.core.upgrade() {
            core.changed();
        }
    }
}
pub(crate) struct PipeWork {
    pub(crate) shared: SharedRef,
    pub(crate) owner: Owner,
    pub(crate) lease: Option<Box<dyn PipeLease>>,
}
pub(crate) fn reserve(
    core: &Arc<Core>,
    scope: &Scope,
    spec: JobSpec,
    run: Option<RunKey>,
    handshake: Moment,
    lease: Box<dyn PipeLease>,
) -> Result<LifecyclePipeSession, AdmissionError> {
    let (inner, completion) =
        admit(core, scope, spec, run.map(PeerBinding::Desktop), handshake, lease, Owner::Desktop)?;
    Ok(LifecyclePipeSession { control: LifecyclePipeControl { inner }, completion })
}
#[cfg(windows)]
mod persistent_scope;
mod submission;
#[cfg(windows)]
pub(crate) use persistent_scope::validate_persistent_scope;
pub(crate) use submission::{PipeAdmission, PreparedPipeLease, admit, admit_owned};
pub(crate) fn failure(stage: &'static str) -> LifecyclePipeFailure {
    LifecyclePipeFailure::native(NativeObservationError::new(
        stage,
        std::io::Error::other("Lifecycle transport invariant rejected"),
    ))
}
pub(crate) fn allocate(
    context: &JobContext,
    shared: &SharedRef,
) -> Result<(), LifecyclePipeFailure> {
    let memory = context.memory();
    if !shared.metadata.installed() {
        let meta = memory.claim(65_536).map_err(|e| {
            LifecyclePipeFailure::native(NativeObservationError::new("pipe_metadata_claim", e))
        })?;
        shared.metadata.install(meta)?;
    }
    let read = buffers::Buffer::new(memory)?;
    let write = buffers::Buffer::new(memory)?;
    let mut s = lock(&shared.state);
    s.read_buffer = Some(read);
    s.write_buffer = Some(write);
    Ok(())
}
pub(crate) fn initial_deadlines(work: &PipeWork) -> Deadlines {
    Deadlines::handshake(work.shared.original_deadline)
}
