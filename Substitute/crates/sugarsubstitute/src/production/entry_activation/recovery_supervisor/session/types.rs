// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Closed fixed ownership and protocol metadata for the actual Root recovery session.
use super::framing::SendPlan;
use super::*;
/// Constructor inputs remain intact on every pre-effect rejection.
pub(in crate::production::entry_activation::recovery_supervisor) struct RootRecoverySessionInput {
    pub(in crate::production::entry_activation::recovery_supervisor) pipe: RecoveryPipeSession,
    pub(in crate::production::entry_activation::recovery_supervisor) grant: RegisteredRecoveryGrant,
    pub(in crate::production::entry_activation::recovery_supervisor) identity: MainIdentity,
    pub(in crate::production::entry_activation::recovery_supervisor) carrier: LaunchTicket,
    pub(in crate::production::entry_activation::recovery_supervisor) root: CaptureRootSource,
    pub(in crate::production::entry_activation::recovery_supervisor) ack: DurableAcknowledgement,
    pub(in crate::production::entry_activation::recovery_supervisor) desktop_watch:
        RecoveryDesktopWatch,
    pub(in crate::production::entry_activation::recovery_supervisor) handler_permit:
        ProcessWatchPermit,
    pub(in crate::production::entry_activation::recovery_supervisor) enrollment:
        Option<LifecycleDesktopEnrollment>,
    pub(in crate::production::entry_activation::recovery_supervisor) memory: MemoryClaim,
    pub(in crate::production::entry_activation::recovery_supervisor) outside: MemoryLease,
}
pub(in crate::production::entry_activation::recovery_supervisor) struct RootRecoverySessionRejection
{
    pub(in crate::production::entry_activation::recovery_supervisor) cause: SessionCause,
    pub(in crate::production::entry_activation::recovery_supervisor) input:
        RootRecoverySessionInput,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production::entry_activation::recovery_supervisor) enum SessionCode {
    Binding,
    Order,
    Exhausted,
    Pending,
    Closed,
    Clock,
    Worker,
    Terminal,
    Revoked,
    Deadline,
}
impl std::fmt::Display for SessionCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Recovery Root session rejected: {self:?}")
    }
}
impl std::error::Error for SessionCode {}
#[derive(Debug)]
pub(in crate::production::entry_activation::recovery_supervisor) enum SessionCause {
    Protocol(SessionCode),
    Wire(RecoveryWireError),
    Pipe(LifecyclePipeError),
    Startup(RecoveryStartupError),
    Admission(AdmissionError),
    Registry(RegistryError),
    Native(NativeObservationError),
    Transport(LifecyclePipeFailure),
    Incident(sugarsubstitute_crash::incident::IncidentStoreError),
    Panic(SessionPanic),
    Remote(sugarsubstitute_contracts::recovery_supervisor::RecoveryFailureCode),
}
impl std::fmt::Display for SessionCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Original recovery Root session failure retained")
    }
}
impl std::error::Error for SessionCause {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Protocol(cause) => Some(cause),
            Self::Wire(cause) => Some(cause),
            Self::Pipe(cause) => Some(cause),
            Self::Startup(cause) => Some(cause),
            Self::Admission(cause) => Some(cause),
            Self::Registry(cause) => Some(cause),
            Self::Native(cause) => Some(cause),
            Self::Transport(cause) => Some(cause),
            Self::Incident(cause) => Some(cause),
            Self::Panic(cause) => Some(cause),
            Self::Remote(code) => {
                let _category = *code;
                None
            }
        }
    }
}
#[derive(Default)]
pub(in crate::production::entry_activation::recovery_supervisor) struct SessionProgress {
    pub(in crate::production::entry_activation::recovery_supervisor) handler_needed: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) desktop_needed: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) handler_lost: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) owner_failed: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) failed: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) retired: bool,
}
pub(in crate::production::entry_activation::recovery_supervisor) struct Funding {
    pub(in crate::production::entry_activation::recovery_supervisor) _memory: MemoryClaim,
}
pub(in crate::production::entry_activation::recovery_supervisor) struct Work {
    pub(in crate::production::entry_activation::recovery_supervisor) desktop_watch:
        Option<RecoveryDesktopWatch>,
    pub(in crate::production::entry_activation::recovery_supervisor) desktop:
        Option<RecoveryEnrolledDesktop>,
    pub(in crate::production::entry_activation::recovery_supervisor) desktop_prepared: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) enrollment:
        Option<LifecycleDesktopEnrollment>,
    pub(in crate::production::entry_activation::recovery_supervisor) handler_permit:
        Option<ProcessWatchPermit>,
    pub(in crate::production::entry_activation::recovery_supervisor) handler:
        Option<RecoveryHandlerWatch>,
    pub(in crate::production::entry_activation::recovery_supervisor) cancelled_ui:
        Option<OwnedJob<ProcessExit, NativeObservationError>>,
    pub(in crate::production::entry_activation::recovery_supervisor) cancelled_handler:
        Option<OwnedJob<ProcessExit, NativeObservationError>>,
    pub(in crate::production::entry_activation::recovery_supervisor) ui_terminal:
        Option<JobCompletion<ProcessExit, NativeObservationError>>,
    pub(in crate::production::entry_activation::recovery_supervisor) handler_terminal:
        Option<JobCompletion<ProcessExit, NativeObservationError>>,
    pub(in crate::production::entry_activation::recovery_supervisor) failure:
        [Option<RegistryError>; 2],
    pub(in crate::production::entry_activation::recovery_supervisor) _funding: Arc<Funding>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production::entry_activation::recovery_supervisor) enum WorkerKind {
    Handler,
    Desktop,
}
impl WorkerKind {
    pub(in crate::production::entry_activation::recovery_supervisor) const fn index(self) -> usize {
        match self {
            Self::Handler => 0,
            Self::Desktop => 1,
        }
    }
}
pub(in crate::production::entry_activation::recovery_supervisor) struct Worker {
    pub(in crate::production::entry_activation::recovery_supervisor) kind: WorkerKind,
    pub(in crate::production::entry_activation::recovery_supervisor) job: OwnedJob<(), SessionCode>,
}
pub(in crate::production::entry_activation::recovery_supervisor) struct Closing {
    pub(in crate::production::entry_activation::recovery_supervisor) counter: CounterDeadline,
    pub(in crate::production::entry_activation::recovery_supervisor) until: Moment,
    pub(in crate::production::entry_activation::recovery_supervisor) sent: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) observers_cancelled: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) control_closed: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) cancellation_failed: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) _revocation:
        Option<NativeWriteRevocation>,
}
pub(in crate::production::entry_activation::recovery_supervisor) struct State {
    pub(in crate::production::entry_activation::recovery_supervisor) execution: ExecutionId,
    pub(in crate::production::entry_activation::recovery_supervisor) pipe: RecoveryPipeSession,
    pub(in crate::production::entry_activation::recovery_supervisor) pipe_scope: ScopeId,
    pub(in crate::production::entry_activation::recovery_supervisor) pipe_spec: JobSpec,
    pub(in crate::production::entry_activation::recovery_supervisor) grant: RegisteredRecoveryGrant,
    pub(in crate::production::entry_activation::recovery_supervisor) identity: MainIdentity,
    pub(in crate::production::entry_activation::recovery_supervisor) carrier: LaunchTicket,
    pub(in crate::production::entry_activation::recovery_supervisor) _root: CaptureRootSource,
    pub(in crate::production::entry_activation::recovery_supervisor) root_identity: [u8; 24],
    pub(in crate::production::entry_activation::recovery_supervisor) ack: DurableAcknowledgement,
    pub(in crate::production::entry_activation::recovery_supervisor) peer:
        Option<VerifiedRecoveryPeer>,
    pub(in crate::production::entry_activation::recovery_supervisor) connected: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) hello: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) prepared: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) plan: SendPlan,
    pub(in crate::production::entry_activation::recovery_supervisor) operation: u64,
    pub(in crate::production::entry_activation::recovery_supervisor) read:
        Option<LifecyclePipeOperationId>,
    pub(in crate::production::entry_activation::recovery_supervisor) admission:
        Option<NativeWriteAdmission>,
    pub(in crate::production::entry_activation::recovery_supervisor) sequence: u64,
    pub(in crate::production::entry_activation::recovery_supervisor) startup_accepted: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) handler_advisory:
        Option<RecoveryProcessIdentity>,
    pub(in crate::production::entry_activation::recovery_supervisor) desktop_advisory:
        Option<RecoveryProcessIdentity>,
    pub(in crate::production::entry_activation::recovery_supervisor) desktop_exit_advisory:
        Option<(RecoveryProcessIdentity, u32)>,
    pub(in crate::production::entry_activation::recovery_supervisor) enrolled_reply: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) subordinate_retired:
        super::framing::RetirementReceipt,
    pub(in crate::production::entry_activation::recovery_supervisor) closing: Option<Closing>,
    pub(in crate::production::entry_activation::recovery_supervisor) pipe_terminal:
        Option<JobCompletion<RecoveryPipeReport, LifecyclePipeFailure>>,
    pub(in crate::production::entry_activation::recovery_supervisor) pipe_verified: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) pipe_retirement:
        Option<RecoveryPipeRetirement>,
    pub(in crate::production::entry_activation::recovery_supervisor) job: Option<Worker>,
    pub(in crate::production::entry_activation::recovery_supervisor) job_terminals:
        [Option<JobCompletion<(), SessionCode>>; 2],
    pub(in crate::production::entry_activation::recovery_supervisor) failures:
        super::failures::Failures,
    pub(in crate::production::entry_activation::recovery_supervisor) exit_mismatch: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) watch_reported: [bool; 2],
    pub(in crate::production::entry_activation::recovery_supervisor) worker_reported: bool,
    pub(in crate::production::entry_activation::recovery_supervisor) last_now: Option<Moment>,
    pub(in crate::production::entry_activation::recovery_supervisor) work: Arc<Mutex<Work>>,
    pub(in crate::production::entry_activation::recovery_supervisor) _funding: Arc<Funding>,
}

impl std::fmt::Debug for RootRecoverySessionRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RootRecoverySessionRejection { owners retained }")
    }
}
impl std::fmt::Display for RootRecoverySessionRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Root recovery control admission rejected")
    }
}
impl std::error::Error for RootRecoverySessionRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
impl RootRecoverySessionRejection {
    /// Return the original typed cause together with every unconsumed admitted owner.
    pub(in crate::production::entry_activation::recovery_supervisor) fn into_parts(
        self,
    ) -> (SessionCause, RootRecoverySessionInput) {
        (self.cause, self.input)
    }
}

#[derive(Clone, Debug)]
pub(in crate::production::entry_activation::recovery_supervisor) struct SessionPanic(
    pub(super) PanicCause,
);
impl std::fmt::Display for SessionPanic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _bounded_original = self.0.as_str();
        f.write_str("Original supervised recovery owner panic retained")
    }
}
impl std::error::Error for SessionPanic {}
impl Closing {
    pub(super) fn latch(
        slot: &mut Option<Self>,
        counter: CounterDeadline,
        until: Moment,
    ) -> Result<bool, SessionCode> {
        if let Some(close) = slot {
            return if close.counter == counter && close.until == until {
                Ok(false)
            } else {
                Err(SessionCode::Binding)
            };
        }
        *slot = Some(Self {
            counter,
            until,
            sent: false,
            observers_cancelled: false,
            control_closed: false,
            cancellation_failed: false,
            _revocation: None,
        });
        Ok(true)
    }
}
