// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Model only external pipe effects while the production owner advances every lifetime.
use super::*;
pub(super) struct Boundary {
    pub(super) signals: Mutex<Option<[Arc<Signal>; 3]>>,
    pub(super) prepared: Gate,
    pub(super) connected: Gate,
    pub(super) reading: Gate,
    pub(super) retiring: Gate,
    pub(super) allow_retire: AtomicBool,
    pub(super) pending_read: AtomicBool,
    pub(super) prepares: AtomicUsize,
    pub(super) closes: AtomicUsize,
    pub(super) adopted: AtomicUsize,
    pub(super) retire_failures: AtomicUsize,
    pub(super) close_failed: Gate,
    pub(super) fail_prepare: AtomicBool,
    pub(super) panic_prepare: AtomicBool,
    pub(super) panic_retire: AtomicBool,
    pub(super) hold_read: AtomicBool,
    pub(super) read_release: Gate,
    pub(super) writes: AtomicUsize,
    pub(super) preparing_write: Gate,
    pub(super) hold_write: AtomicBool,
    pub(super) write_release: Gate,
    pub(super) fail_write_issue: AtomicBool,
    pub(super) fail_write_completion: AtomicBool,
    pub(super) early_read: AtomicBool,
    pub(super) write_quiesced: AtomicUsize,
    pub(super) fail_write_quiesce: AtomicBool,
    pub(super) writing: Gate,
    pub(super) pending_write: AtomicBool,
    pub(super) fail_read: AtomicBool,
    pub(super) peer_closed: AtomicBool,
    pub(super) retired_write: Mutex<Option<u32>>,
    pub(super) write_polls: AtomicUsize,
    pub(super) panic_read: AtomicBool,
    pub(super) panic_write: AtomicBool,
    pub(super) memory: Mutex<Option<MemoryLease>>,
    pub(super) fatal_auth: AtomicBool,
    pub(super) fatal_message: AtomicBool,
    pub(super) reject_message: AtomicBool,
    pub(super) message_checks: AtomicUsize,
    pub(super) restoration_failed: AtomicBool,
    pub(super) owner_thread: Mutex<Option<std::thread::ThreadId>>,
}
impl Boundary {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            signals: Mutex::new(None),
            prepared: Gate::default(),
            connected: Gate::default(),
            reading: Gate::default(),
            retiring: Gate::default(),
            allow_retire: AtomicBool::new(true),
            pending_read: AtomicBool::new(false),
            prepares: AtomicUsize::new(0),
            closes: AtomicUsize::new(0),
            adopted: AtomicUsize::new(0),
            retire_failures: AtomicUsize::new(0),
            close_failed: Gate::default(),
            fail_prepare: AtomicBool::new(false),
            panic_prepare: AtomicBool::new(false),
            panic_retire: AtomicBool::new(false),
            hold_read: AtomicBool::new(false),
            read_release: Gate::default(),
            writes: AtomicUsize::new(0),
            preparing_write: Gate::default(),
            hold_write: AtomicBool::new(false),
            write_release: Gate::default(),
            fail_write_issue: AtomicBool::new(false),
            fail_write_completion: AtomicBool::new(false),
            early_read: AtomicBool::new(false),
            write_quiesced: AtomicUsize::new(0),
            fail_write_quiesce: AtomicBool::new(false),
            writing: Gate::default(),
            pending_write: AtomicBool::new(false),
            fail_read: AtomicBool::new(false),
            peer_closed: AtomicBool::new(false),
            retired_write: Mutex::new(None),
            write_polls: AtomicUsize::new(0),
            panic_read: AtomicBool::new(false),
            panic_write: AtomicBool::new(false),
            memory: Mutex::new(None),
            fatal_auth: AtomicBool::new(false),
            fatal_message: AtomicBool::new(false),
            reject_message: AtomicBool::new(false),
            message_checks: AtomicUsize::new(0),
            restoration_failed: AtomicBool::new(false),
            owner_thread: Mutex::new(None),
        })
    }
    pub(super) fn wake(&self, role: Role) -> TestResult {
        lock(&self.signals).as_ref().ok_or("Pipe not prepared")?[role.index()].complete();
        Ok(())
    }
}
pub(super) struct Lease(pub(super) Arc<Boundary>);
impl PipeLease for Lease {
    fn adopt(&mut self, _: PeerInput) {
        self.0.adopted.fetch_add(1, Ordering::AcqRel);
    }
    fn prepare(
        &mut self,
        signals: [Arc<Signal>; 3],
        context: &JobContext,
        _: Moment,
    ) -> Result<usize, LifecyclePipeFailure> {
        *lock(&self.0.memory) = Some(context.memory().clone());
        *lock(&self.0.signals) = Some(signals);
        *lock(&self.0.owner_thread) = Some(std::thread::current().id());
        self.0.prepares.fetch_add(1, Ordering::AcqRel);
        self.0.prepared.open();
        if self.0.panic_prepare.load(Ordering::Acquire) {
            std::panic::resume_unwind(Box::new("Secret pipe owner panic".to_owned()));
        }
        if self.0.fail_prepare.load(Ordering::Acquire) {
            return Err(failure("partial_pipe_creation"));
        }
        Ok(19)
    }
    fn run(&self) -> Option<RunKey> {
        run().ok()
    }
    fn deadline(&self) -> Option<LifecyclePipeDeadline> {
        LifecyclePipeDeadline::new(1000, 1000).ok()
    }
    fn capability(&self) -> Option<[u8; 32]> {
        Some([7; 32])
    }
    fn connect(&mut self) -> Result<Progress, LifecyclePipeFailure> {
        self.0.connected.open();
        Ok(Progress::Complete(0))
    }
    fn read(&mut self, bytes: &mut [u8]) -> Result<Progress, LifecyclePipeFailure> {
        if self.0.fail_read.load(Ordering::Acquire) {
            return Err(failure("concurrent_read_failure"));
        }
        bytes[..5].copy_from_slice(b"Hello");
        self.0.reading.open();
        if self.0.panic_read.swap(false, Ordering::AcqRel) {
            std::panic::resume_unwind(Box::new("Pending read issue panic"));
        }
        if self.0.hold_read.load(Ordering::Acquire) {
            self.0.read_release.wait().map_err(|_| failure("held_read"))?;
        }
        Ok(if self.0.pending_read.load(Ordering::Acquire) {
            Progress::Pending
        } else {
            Progress::Complete(5)
        })
    }
    fn write(
        &mut self,
        bytes: &[u8],
        issue: WriteIssue<'_>,
    ) -> Result<WriteDispatch<Progress>, LifecyclePipeFailure> {
        self.0.preparing_write.open();
        if self.0.hold_write.load(Ordering::Acquire) {
            self.0.write_release.wait().map_err(|_| failure("held_write"))?;
        }
        let result = issue.execute(|| {
            self.0.writes.fetch_add(1, Ordering::AcqRel);
            if self.0.panic_write.swap(false, Ordering::AcqRel) {
                std::panic::resume_unwind(Box::new("Pending write issue panic"));
            }
            if self.0.fail_write_issue.load(Ordering::Acquire) {
                return Err(failure("write_issue_rejected"));
            }
            if self.0.early_read.load(Ordering::Acquire) {
                self.0.wake(Role::Read).map_err(|_| failure("early_read"))?;
            }
            Ok(if self.0.pending_write.load(Ordering::Acquire) {
                Progress::Pending
            } else {
                Progress::Complete(bytes.len() as u32)
            })
        })?;
        self.0.writing.open();
        if matches!(result, WriteDispatch::Issued(_))
            && self.0.fail_write_completion.load(Ordering::Acquire)
        {
            return Err(failure("write_completion_rejected"));
        }
        Ok(result)
    }
    fn poll(&mut self, role: Role) -> Result<Progress, LifecyclePipeFailure> {
        if role == Role::Read && self.0.peer_closed.load(Ordering::Acquire) {
            return Err(failure("passive_peer_closed"));
        }
        if role == Role::Write {
            self.0.write_polls.fetch_add(1, Ordering::AcqRel);
        }
        Ok(Progress::Complete(5))
    }
    fn quiesce(&mut self, role: Role) -> Result<(), LifecyclePipeFailure> {
        if role == Role::Write {
            self.0.write_quiesced.fetch_add(1, Ordering::AcqRel);
            if self.0.fail_write_quiesce.load(Ordering::Acquire) {
                return Err(failure("write_callback_quiesce"));
            }
        }
        Ok(())
    }
    fn verify_message(&mut self) -> Result<(), LifecyclePipeFailure> {
        self.0.message_checks.fetch_add(1, Ordering::AcqRel);
        if self.0.fatal_message.load(Ordering::Acquire) {
            self.0.restoration_failed.store(true, Ordering::Release);
            let mut error = failure("failed_identity_restore");
            error.identity_unrestored = true;
            return Err(error);
        }
        if self.0.reject_message.load(Ordering::Acquire) {
            return Err(failure("pipe_message_identity_mismatch"));
        }
        Ok(())
    }
    fn authenticate(
        &mut self,
        candidate: &[u8; 32],
    ) -> Result<VerifiedLifecyclePeer, LifecyclePipeFailure> {
        if self.0.fatal_auth.load(Ordering::Acquire) {
            self.0.restoration_failed.store(true, Ordering::Release);
            let mut error = failure("failed_identity_restore");
            error.identity_unrestored = true;
            return Err(error);
        }
        if candidate != &[7; 32] {
            return Err(failure("fake_credential_rejection"));
        }
        Ok(VerifiedLifecyclePeer {
            run: run().map_err(|_| failure("fake_run"))?,
            process: None,
            pid: 71,
            birth: 99,
            session: 1,
        })
    }
    fn take_completed_write(&mut self) -> Option<u32> {
        lock(&self.0.retired_write).take()
    }
    fn retire(&mut self, pinned: bool) -> Result<bool, LifecyclePipeFailure> {
        self.0.retiring.open();
        if self.0.panic_retire.swap(false, Ordering::AcqRel) {
            std::panic::resume_unwind(Box::new("Secret retirement panic".to_owned()));
        }
        if pinned {
            return Ok(false);
        }
        if self
            .0
            .retire_failures
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1))
            .is_ok()
        {
            self.0.close_failed.open();
            return Err(failure("pipe_checked_close"));
        }
        if !self.0.allow_retire.load(Ordering::Acquire) {
            return Ok(false);
        }
        self.0.closes.fetch_add(1, Ordering::AcqRel);
        Ok(true)
    }
}
