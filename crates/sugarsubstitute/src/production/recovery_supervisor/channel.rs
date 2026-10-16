// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own one actual authenticated Root recovery session and its charged startup receiver.
#[path = "channel/enrollment.rs"]
mod enrollment;
#[path = "channel/incoming.rs"]
mod incoming;
#[path = "channel/receipt.rs"]
mod receipt;
#[path = "channel/receiving.rs"]
mod receiving;
#[path = "channel/sending.rs"]
mod sending;
#[path = "channel/terminal.rs"]
pub(super) mod terminal;
use crate::job_sequence::JobSequence;
use std::{error::Error, num::NonZeroUsize, sync::Arc};
use sugarsubstitute_contracts::{
    execution::*,
    recovery_supervisor::{RecoveryGrantKey, RecoveryProcessIdentity, wire::*},
};
use sugarsubstitute_execution::{ExecutionClient, windows_recovery_supervisor::*};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
type ReceiverJob = OwnedJob<RecoveryStartupAssembly, RecoveryStartupError>;
type ReceiverTerminal = JobCompletion<(), terminal::ReceiverCause>;
type PipeTerminal = JobCompletion<RecoveryPipeReport, LifecyclePipeFailure>;
/// One completed original write. These facts never grant peer processing or native authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Sent {
    Hello,
    StartupAccepted,
    CaptureRegistered,
    DesktopCreated,
    DesktopExited,
    Retired,
}
/// Closed lifecycle messages emitted only by this subordinate's actual owners.
pub(super) enum Outgoing {
    StartupAccepted,
    CaptureRegistered { supervisor: RecoveryProcessIdentity, handler: RecoveryProcessIdentity },
    DesktopCreated(RecoveryProcessIdentity),
    DesktopExited { process: RecoveryProcessIdentity, status: u32 },
    Retired,
}
struct Write {
    receipt: receipt::Receipt,
    sent: Sent,
    admission: NativeWriteAdmission,
    returned: Option<LifecyclePipeWriteBuffer>,
}

/// Keep original session, receiver, partial frame and completion guards until actual retirement.
/// Numeric command arguments cannot manufacture this source or a successful registration.
pub(super) struct Channel {
    session: RecoveryPipeSession,
    receiver: Option<ReceiverJob>,
    receiver_original: Option<(ScopeId, JobSpec)>,
    receiver_terminal: Option<ReceiverTerminal>,
    assembly: Option<RecoveryStartupAssembly>,
    startup: Option<RecoveryStartup>,
    pipe_original: (ScopeId, JobSpec),
    pipe_terminal: Option<PipeTerminal>,
    capability: Option<LifecycleClientCapability>,
    key: Option<RecoveryGrantKey>,
    connected: bool,
    authenticated: bool,
    hello_written: bool,
    startup_transferred: bool,
    closing: bool,
    operation: u64,
    rx_sequence: u64,
    tx_sequence: u64,
    read: Option<LifecyclePipeOperationId>,
    received: Option<ReceivedLifecycleFrame>,
    write: Option<Write>,
    sent: Option<Sent>,
}
impl Channel {
    /// Reserve the existing persistent native pipe before adopting either inherited section.
    /// Rejection preserves the identical lexical pair with its caller.
    pub(super) fn reserve(
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        input: &mut Option<InheritedRecoveryBootstrap>,
        original: LifecyclePipeDeadline,
    ) -> Result<Self> {
        if input.is_none() {
            return Err("Recovery inherited pair missing".into());
        }
        let spec = JobSpec {
            id: jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            deadline: None,
            reservation_bytes: MIN_LIFECYCLE_PIPE_BYTES,
            replacement: None,
        };
        let permit = client.reserve_recovery_pipe_client(scope, spec, original)?;
        let pair = input.take().ok_or("Admitted recovery pair disappeared")?;
        let session = permit.bind(pair).ok_or("Admitted recovery session missing")?;
        Ok(Self {
            session,
            receiver: None,
            receiver_original: None,
            receiver_terminal: None,
            assembly: None,
            startup: None,
            pipe_original: (scope.id(), spec),
            pipe_terminal: None,
            capability: None,
            key: None,
            connected: false,
            authenticated: false,
            hello_written: false,
            startup_transferred: false,
            closing: false,
            operation: 0,
            rx_sequence: 0,
            tx_sequence: 0,
            read: None,
            received: None,
            write: None,
            sent: None,
        })
    }
    /// Drain fixed real transport observations, then advance eligible receiver work without waiting.
    pub(super) fn activity(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        max: NonZeroUsize,
    ) -> Result<()> {
        self.observe_terminal()?;
        for observation in self.session.drain(max) {
            self.observe(observation)?;
        }
        self.observe_receiver()?;
        if self.closing || self.pipe_terminal.is_some() {
            return Ok(());
        }
        if self.authenticated && !self.hello_written && self.write.is_none() {
            self.hello()?;
        }
        if self.hello_written && self.receiver.is_none() && self.receiver_original.is_none() {
            let spec = JobSpec {
                id: jobs.allocate()?,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(self.original_until()),
                reservation_bytes: 4 * 1024 * 1024,
                replacement: None,
            };
            self.receiver = Some(client.submit_recovery_startup_assembly(
                scope,
                spec,
                self.session.control(),
            )?);
            self.receiver_original = Some((scope.id(), spec));
        }
        if self.assembly.is_some() {
            self.receive_startup()?;
        }
        if self.hello_written
            && (self.assembly.is_some() || self.startup_transferred || self.startup.is_some())
            && self.read.is_none()
            && self.received.is_none()
        {
            let operation = self.next_operation()?;
            if self.startup_transferred || self.startup.is_some() {
                self.session.control().read_passive(operation)?;
            } else {
                self.session.control().read(operation, self.original_until())?;
            }
            self.read = Some(operation);
        }
        Ok(())
    }
    /// Transfer the actual successful receiver output once; original completion custody stays here.
    pub(super) fn take_startup(&mut self) -> Option<RecoveryStartup> {
        let value = self.startup.take()?;
        self.startup_transferred = true;
        Some(value)
    }
    /// Return matching actual authenticated Root binding facts, never a delegated capability.
    pub(super) const fn key(&self) -> Option<RecoveryGrantKey> {
        self.key
    }
    /// Return the unchanged single conservative translation of the Root admission counter.
    pub(super) fn original_until(&self) -> Moment {
        self.session.control().original_deadline()
    }
    /// Borrow the original authenticated frame without reconstructing its source custody.
    pub(super) fn control_frame(&self) -> Option<&ReceivedLifecycleFrame> {
        if self.startup_transferred { self.received.as_ref() } else { None }
    }
    /// Borrow the exact real control for source-bound native enrollment validation.
    pub(super) fn control(&self) -> &RecoveryPipeControl {
        self.session.control()
    }
    /// Take the actual full native write observation once.
    pub(super) fn take_sent(&mut self) -> Option<Sent> {
        self.sent.take()
    }
    /// Fence admission and cancel actual pending work without claiming physical settlement.
    pub(super) fn request_close(&mut self) {
        self.closing = true;
        self.capability = None;
        self.received = None;
        self.assembly = None;
        self.startup = None;
        if let Some(receiver) = &self.receiver {
            receiver.cancel();
        }
        if let Some(write) = &mut self.write {
            let _revocation = write.admission.revoke();
        }
        self.session.control().close();
    }
    /// Actual terminal publication proves native transport retirement, independently of family cleanup.
    pub(super) fn settled(&self) -> bool {
        self.pipe_terminal.is_some()
            && (self.receiver_original.is_none() || self.receiver_terminal.is_some())
    }
    fn next_operation(&mut self) -> Result<LifecyclePipeOperationId> {
        self.operation = self.operation.checked_add(1).ok_or("Recovery operation exhausted")?;
        Ok(LifecyclePipeOperationId::new(self.operation)?)
    }
}
