// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own closed lifecycle transport sessions and fixed command slots after real admission.
mod admission;
mod buffers;
mod control;
pub(crate) mod enrollment;
mod issue;
#[cfg(test)]
mod tests;
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
pub use buffers::{LifecyclePipeWriteBuffer, ReceivedLifecycleFrame};
pub use control::{
    LifecyclePipeBootstrap, LifecyclePipeControl, LifecyclePipeSession, LifecyclePipeWriteRejection,
};
pub(crate) use issue::{WriteDispatch, WriteIssue};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, Weak},
};
use sugarsubstitute_contracts::{
    execution::{AdmissionError, JobContext, JobSpec, MemoryClaim, Moment, OwnedJob, Scope},
    lifecycle::{ProcessKey, RunKey},
};
pub use types::*;

pub(crate) use crate::native_pipe::Progress;
pub(crate) enum PeerInput {
    #[cfg(windows)]
    Owned { handle: std::os::windows::io::OwnedHandle, process: Option<ProcessKey> },
    #[cfg(windows)]
    Inherited { section: usize },
    #[cfg(test)]
    Simulated,
}
pub(crate) trait PipeLease: Send {
    fn adopt(&mut self, input: PeerInput);
    fn prepare(
        &mut self,
        signals: [Arc<Signal>; 3],
        context: &JobContext,
        handshake: Moment,
    ) -> Result<usize, LifecyclePipeFailure>;
    fn run(&self) -> Option<RunKey>;
    fn deadline(&self) -> Option<LifecyclePipeDeadline>;
    fn capability(&self) -> Option<[u8; 32]>;
    fn connect(&mut self) -> Result<Progress, LifecyclePipeFailure>;
    fn read(&mut self, bytes: &mut [u8]) -> Result<Progress, LifecyclePipeFailure>;
    fn write(
        &mut self,
        bytes: &[u8],
        issue: WriteIssue<'_>,
    ) -> Result<WriteDispatch<Progress>, LifecyclePipeFailure>;
    fn poll(&mut self, role: Role) -> Result<Progress, LifecyclePipeFailure>;
    fn quiesce(&mut self, role: Role) -> Result<(), LifecyclePipeFailure>;
    fn verify_message(&mut self) -> Result<(), LifecyclePipeFailure>;
    fn authenticate(
        &mut self,
        candidate: &[u8; 32],
    ) -> Result<VerifiedLifecyclePeer, LifecyclePipeFailure>;
    fn take_completed_write(&mut self) -> Option<u32>;
    fn retire(&mut self, launch_pinned: bool) -> Result<bool, LifecyclePipeFailure>;
}
pub(crate) struct Command {
    pub(crate) operation: LifecyclePipeOperationId,
    pub(crate) deadline: Moment,
}
/// Preserve finite command facts or an authenticated passive receive with no idle timer.
#[derive(Clone, Copy)]
pub(crate) enum ReadDeadline {
    Original(Moment),
    Passive,
}
/// Bind one monotonic read identity to the policy admitted by its session control.
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
    pub(crate) stopped: bool,
    pub(crate) prepared: bool,
    pub(crate) connected: bool,
    pub(crate) authenticated: bool,
    pub(crate) auth_busy: bool,
    pub(crate) run: Option<RunKey>,
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
    pub(crate) read_buffer: Option<buffers::Buffer>,
    pub(crate) write_buffer: Option<buffers::Buffer>,
    pub(crate) observations: VecDeque<LifecyclePipeObservation>,
    pub(crate) received: u64,
    pub(crate) written: u64,
}
impl State {
    fn new(run: Option<RunKey>) -> Self {
        Self {
            stopped: false,
            prepared: false,
            connected: false,
            authenticated: false,
            auth_busy: false,
            run,
            peer: None,
            enrolled: false,
            enrollment_taken: false,
            inherited: 0,
            pin: false,
            transferred: false,
            input_pin: run.is_none(),
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
    pub(crate) original_deadline: Moment,
    pub(crate) state: Mutex<State>,
    pub(crate) memory: Mutex<Option<MemoryClaim>>,
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
    pub(crate) shared: Arc<Shared>,
    pub(crate) job: Arc<Job<LifecyclePipeReport, LifecyclePipeFailure>>,
    pub(crate) lease: Option<Box<dyn PipeLease>>,
    pub(crate) handshake: Moment,
}
pub(crate) fn reserve(
    core: &Arc<Core>,
    scope: &Scope,
    spec: JobSpec,
    run: Option<RunKey>,
    handshake: Moment,
    lease: Box<dyn PipeLease>,
) -> Result<LifecyclePipeSession, AdmissionError> {
    if spec.reservation_bytes < MIN_LIFECYCLE_PIPE_BYTES {
        return Err(AdmissionError::NativeReservationTooSmall {
            requested_bytes: spec.reservation_bytes,
            minimum_bytes: MIN_LIFECYCLE_PIPE_BYTES,
        });
    }
    if handshake <= core.clock.now() {
        return Err(AdmissionError::DeadlineExpired { deadline: handshake, now: core.clock.now() });
    }
    core.admit(scope, spec, RequestKind::Native, |context, cancel, admitted| {
        let job = Job::new(scope.id(), spec, admitted, cancel, Arc::downgrade(core));
        let shared = Arc::new(Shared {
            original_deadline: handshake,
            state: Mutex::new(State::new(run)),
            memory: Mutex::new(None),
            core: Arc::downgrade(core),
            id: spec.id,
        });
        let queued = QueuedJob {
            context,
            job: job.clone(),
            work: Work::Native(crate::native::NativeWork::LifecyclePipe(PipeWork {
                shared: shared.clone(),
                job: job.clone(),
                lease: Some(lease),
                handshake,
            })),
        };
        let session = LifecyclePipeSession {
            control: LifecyclePipeControl { shared, job: job.clone() },
            completion: OwnedJob::new(scope.id(), spec, Box::new(JobHandle(job))),
        };
        (queued, session)
    })
}
pub(crate) fn failure(stage: &'static str) -> LifecyclePipeFailure {
    LifecyclePipeFailure::native(NativeObservationError::new(
        stage,
        std::io::Error::other("Lifecycle transport invariant rejected"),
    ))
}
pub(crate) fn allocate(context: &JobContext, shared: &Shared) -> Result<(), LifecyclePipeFailure> {
    let memory = context.memory();
    let meta = memory.claim(65_536).map_err(|e| {
        LifecyclePipeFailure::native(NativeObservationError::new("pipe_metadata_claim", e))
    })?;
    *lock(&shared.memory) = Some(meta);
    let read = buffers::Buffer::new(memory)?;
    let write = buffers::Buffer::new(memory)?;
    let mut s = lock(&shared.state);
    s.read_buffer = Some(read);
    s.write_buffer = Some(write);
    Ok(())
}
pub(crate) fn initial_deadlines(work: &PipeWork) -> Deadlines {
    Deadlines::handshake(work.handshake)
}
