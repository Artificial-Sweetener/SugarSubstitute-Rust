// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain original startup close errors with inert, closed diagnostic facts.

use std::{error::Error, fmt, io::ErrorKind};
use sugarsubstitute_contracts::{
    execution::{ExecutionError, TerminalKind, TimeError},
    transcript::TranscriptError,
};
#[path = "startup_failure/child_causes.rs"]
mod child_causes;
#[path = "startup_failure/vocabulary.rs"]
mod vocabulary;
pub use child_causes::{
    StartupChallengeCause, StartupOutputCause, StartupPipeCause, StartupTimeCause,
};
pub use vocabulary::{StartupCanvasMessageCause, StartupObservationKind};

/// The first authored close operation that actually returned the retained error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupCloseStage {
    /// No close owner supplied stage information.
    None,
    /// The original capture claimant's cleanup observation failed.
    CaptureObservation,
    /// The original close budget could not be adopted or measured.
    ShutdownBudget,
    /// Original authenticated exit-intent preparation or sending failed.
    ExitIntent,
    /// The application startup policy rejected close intent.
    PolicyClose,
    /// The actual child rejected the policy's cleanup effect.
    PolicyApply,
    /// Governed render preparation rejected retirement.
    RenderRetirement,
    /// The application canvas rejected its original retirement cutoff.
    CanvasRetirement,
    /// The original deadline could not be projected for native waiting.
    DeadlineRefresh,
}

/// Passive state of the existing crash-capture cleanup snapshot, never a readiness grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupCaptureState {
    /// No composing close owner observed a capture snapshot.
    Unobserved,
    /// The actual child has no admitted capture session.
    Absent,
    /// The admitted capture session has not requested cleanup.
    Dormant,
    /// Its first original clock observation remains in progress.
    Requested,
    /// The original execution and native-counter budget is available.
    Latched,
    /// An original execution budget exists but its native encoding failed.
    CounterFailed,
    /// The original claimant could not obtain an execution budget.
    ClockUnavailable,
}

/// Supported directly observed native cause stages; unknown authored stages remain opaque.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupNativeCause {
    /// Capture execution-clock observation.
    CaptureClock,
    /// Capture ten/fifteen-second budget construction.
    CaptureBudget,
    /// The actual capture execution owner was unavailable.
    CaptureOwnerUnavailable,
    /// Capture cleanup counter arithmetic.
    CaptureCounterArithmetic,
    /// The actual native counter query.
    CounterQuery,
    /// The actual native counter frequency query.
    CounterFrequency,
    /// The native provider returned nonpositive counter facts.
    CounterPositive,
    /// The execution clock used by the native counter conversion.
    CounterClock,
    /// Counter positivity, frequency or expiry conversion.
    CounterConversion,
    /// Native counter deadline arithmetic.
    CounterArithmetic,
    /// Native counter deadline shape.
    CounterShape,
    /// A different direct native observation stage.
    Other,
    /// The native owner retained a supervised panic.
    OwnerPanic,
}

/// Closed direct lifecycle request rejection categories, without message text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupRequestCause {
    /// Wire registration is incomplete.
    Registration,
    /// Application request admission has closed.
    Closed,
    /// Original bounded request slots remain occupied.
    Capacity,
    /// Original request identities cannot advance.
    Exhausted,
    /// Exact original request identity or deadlines differ.
    Identity,
    /// Native clock frequency differs.
    Frequency,
    /// Actual request lifecycle ordering differs.
    Order,
    /// The original request cutoff has expired.
    Deadline,
}

/// Closed directly returned bootstrap rejection categories; original causes remain owned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupBootstrapCause {
    /// Bootstrap lifecycle state was rejected.
    State,
    /// Bootstrap original identity was rejected.
    Identity,
    /// Actual channel consumption failed.
    Transport,
    /// Actual capture consumption failed.
    Capture,
    /// Actual configuration projection failed.
    Configuration,
    /// The original bootstrap cutoff expired.
    Deadline,
}

/// Closed cause projection, independent of formatting or arbitrary source callbacks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupCauseCode {
    /// The direct original cause has no supported concrete projection.
    Opaque,
    /// Direct operating-system error number.
    Os(i32),
    /// Direct Windows HRESULT.
    Hresult(i32),
    /// Direct IO timeout.
    TimedOut,
    /// Direct IO disconnection.
    Disconnected,
    /// Direct IO access rejection.
    Denied,
    /// Direct IO input rejection.
    InvalidInput,
    /// Other direct IO failure.
    Io,
    /// Actual supported native stage and optional direct Win32 code.
    Native(StartupNativeCause, Option<u32>),
    /// Actual typed request rejection.
    Request(StartupRequestCause),
    /// Direct bootstrap category; no underlying arbitrary cause is traversed.
    Bootstrap(StartupBootstrapCause),
    /// Actual typed transcript rejection; original output completion custody stays owned.
    Transcript(TranscriptError),
    /// A native canvas message could not be bound for delivery.
    CanvasMessage(StartupCanvasMessageCause),
    /// Direct lifecycle pipe command rejection.
    Pipe(StartupPipeCause),
    /// Rejected coordinator UI watchdog challenge exchange.
    Challenge(StartupChallengeCause),
    /// Managed output ingestion failure other than a transcript rejection.
    Output(StartupOutputCause),
    /// Invalid monotonic clock observation or unrepresentable deadline.
    Time(StartupTimeCause),
    /// The execution owner failed an infrastructure operation such as its clock.
    Execution,
    /// A coordinator frame failed closed wire decoding.
    Protocol,
    /// The main window request job ended without success.
    MainTerminal(TerminalKind),
}

impl StartupCauseCode {
    /// Inspect supported direct concrete causes without formatting or walking their sources.
    pub fn direct(cause: &(dyn Error + 'static)) -> Self {
        if let Some(transcript) = cause.downcast_ref::<TranscriptError>() {
            return Self::Transcript(*transcript);
        }
        if let Some(message) = cause.downcast_ref::<crate::CanvasNativeReplyError>() {
            return Self::CanvasMessage(match message {
                crate::CanvasNativeReplyError::Storage => StartupCanvasMessageCause::Storage,
                crate::CanvasNativeReplyError::ForeignNavigation => {
                    StartupCanvasMessageCause::ForeignNavigation
                }
                crate::CanvasNativeReplyError::Identity(_) => StartupCanvasMessageCause::Identity,
            });
        }
        #[cfg(target_os = "windows")]
        if let Some(native) = cause.downcast_ref::<windows::core::Error>() {
            return Self::Hresult(native.code().0);
        }
        if let Some(io) = cause.downcast_ref::<std::io::Error>() {
            if let Some(code) = io.raw_os_error() {
                return Self::Os(code);
            }
            return match io.kind() {
                ErrorKind::TimedOut => Self::TimedOut,
                ErrorKind::BrokenPipe
                | ErrorKind::ConnectionAborted
                | ErrorKind::ConnectionReset
                | ErrorKind::NotConnected => Self::Disconnected,
                ErrorKind::PermissionDenied => Self::Denied,
                ErrorKind::InvalidData | ErrorKind::InvalidInput => Self::InvalidInput,
                _ => Self::Io,
            };
        }
        if let Some(time) = cause.downcast_ref::<TimeError>() {
            return Self::Time(match time {
                TimeError::Overflow => StartupTimeCause::Overflow,
                TimeError::Regression => StartupTimeCause::Regression,
            });
        }
        if cause.downcast_ref::<ExecutionError>().is_some() {
            return Self::Execution;
        }
        Self::Opaque
    }
}

impl fmt::Display for StartupCauseCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native(stage, Some(code)) => write!(formatter, "NativeOs({stage:?},{code})"),
            Self::Native(stage, None) => write!(formatter, "Native({stage:?})"),
            Self::Request(kind) => write!(formatter, "Request({kind:?})"),
            Self::Bootstrap(kind) => write!(formatter, "Bootstrap({kind:?})"),
            Self::Transcript(kind) => write!(formatter, "Transcript({kind:?})"),
            Self::CanvasMessage(kind) => write!(formatter, "CanvasMessage({kind:?})"),
            _ => write!(formatter, "{self:?}"),
        }
    }
}

/// Own the unchanged first error while carrying only inert authored close metadata.
pub struct StartupCloseFailure {
    stage: StartupCloseStage,
    capture: StartupCaptureState,
    code: StartupCauseCode,
    cause: Box<dyn Error>,
}

impl StartupCloseFailure {
    /// Enrich an actual failed operation; an already enriched first error remains unchanged.
    /// The caller derives facts from its actual operation and passive owned snapshot.
    /// This transfers the same original Box and invokes no providers or error callbacks.
    pub fn retain(
        stage: StartupCloseStage,
        capture: StartupCaptureState,
        code: StartupCauseCode,
        cause: Box<dyn Error>,
    ) -> Box<dyn Error> {
        if cause.downcast_ref::<Self>().is_some() {
            return cause;
        }
        Box::new(Self { stage, capture, code, cause })
    }
    /// Return the first actual authored close stage.
    pub const fn stage(&self) -> StartupCloseStage {
        self.stage
    }
    /// Return its original passive snapshot category.
    pub const fn capture(&self) -> StartupCaptureState {
        self.capture
    }
    /// Return its direct sealed cause projection.
    pub const fn code(&self) -> StartupCauseCode {
        self.code
    }
    /// Borrow the unchanged original cause without invoking its source callback.
    pub fn original(&self) -> &(dyn Error + 'static) {
        self.cause.as_ref()
    }
}

impl fmt::Debug for StartupCloseFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("StartupCloseFailure")
    }
}
impl fmt::Display for StartupCloseFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Original startup close operation failed")
    }
}
impl Error for StartupCloseFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

#[cfg(test)]
#[path = "startup_failure/tests.rs"]
mod tests;
