// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project passive capture facts and concrete original native/control causes.
use super::*;

impl ProductionChild {
    /// Inspect concrete owner envelopes; never sample capture or walk arbitrary sources.
    pub(in crate::production) fn startup_cause_code(
        cause: &(dyn Error + 'static),
    ) -> sugarsubstitute_desktop::StartupCauseCode {
        cause_code(cause)
    }
    /// Borrow the original snapshot and concrete owned causes without providers.
    pub(in crate::production) fn close_diagnostic(
        &self,
        cause: &(dyn Error + 'static),
    ) -> (sugarsubstitute_desktop::StartupCaptureState, sugarsubstitute_desktop::StartupCauseCode)
    {
        let snapshot = capture_category(self.bootstrap.capture_cleanup_snapshot());
        (snapshot, cause_code(cause))
    }
}

/// Project the existing snapshot value; this never invokes a capture/provider operation.
pub(super) fn capture_category(
    snapshot: Option<CaptureCleanupSnapshot>,
) -> sugarsubstitute_desktop::StartupCaptureState {
    use sugarsubstitute_desktop::StartupCaptureState as State;
    match snapshot {
        None => State::Absent,
        Some(CaptureCleanupSnapshot::Dormant) => State::Dormant,
        Some(CaptureCleanupSnapshot::Requested) => State::Requested,
        Some(CaptureCleanupSnapshot::Latched { counter: Ok(_), .. }) => State::Latched,
        Some(CaptureCleanupSnapshot::Latched { counter: Err(_), .. }) => State::CounterFailed,
        Some(CaptureCleanupSnapshot::ClockUnavailable(_)) => State::ClockUnavailable,
    }
}

/// Inspect concrete native/control owners without walking arbitrary sources.
pub(super) fn cause_code(
    cause: &(dyn Error + 'static),
) -> sugarsubstitute_desktop::StartupCauseCode {
    use sugarsubstitute_desktop::{
        StartupCauseCode as Code, StartupNativeCause as Native, StartupRequestCause as Request,
    };
    if let Some(step) = cause.downcast_ref::<sugarsubstitute_desktop::StartupChildFailure>() {
        return cause_code(step.original());
    }
    if let Some(inner) = diagnostics::preparation_cause(cause) {
        return cause_code(inner);
    }
    if let Some(code) = channel::cause_code(cause)
        .or_else(|| main::cause_code(cause))
        .or_else(|| child_cause_code(cause))
    {
        return code;
    }
    if let Some(native) =
        sugarsubstitute_execution::windows::NativeObservationError::from_execution_cause(cause)
    {
        let stage = if native.contains_owner_panic() {
            Native::OwnerPanic
        } else {
            match native.stage() {
                "capture_cleanup_clock" => Native::CaptureClock,
                "capture_cleanup_budget" => Native::CaptureBudget,
                "capture_cleanup_owner_unavailable" => Native::CaptureOwnerUnavailable,
                "capture_cleanup_counter_arithmetic" => Native::CaptureCounterArithmetic,
                "execution_qpc_counter" => Native::CounterQuery,
                "execution_qpc_frequency" => Native::CounterFrequency,
                "execution_positive_counter" => Native::CounterPositive,
                "execution_counter_clock" => Native::CounterClock,
                "counter_deadline_conversion" => Native::CounterConversion,
                "counter_deadline_arithmetic" => Native::CounterArithmetic,
                "counter_deadline_shape" => Native::CounterShape,
                _ => Native::Other,
            }
        };
        Code::Native(stage, native.win32_code())
    } else if let Some(request) = cause.downcast_ref::<requests::RequestError>() {
        Code::Request(match request {
            requests::RequestError::Registration => Request::Registration,
            requests::RequestError::Closed => Request::Closed,
            requests::RequestError::Capacity => Request::Capacity,
            requests::RequestError::Exhausted => Request::Exhausted,
            requests::RequestError::Identity => Request::Identity,
            requests::RequestError::Frequency => Request::Frequency,
            requests::RequestError::Order => Request::Order,
            requests::RequestError::Deadline => Request::Deadline,
        })
    } else if let Some(issue) = cause.downcast_ref::<bootstrap::ChildBootstrapIssue>() {
        use sugarsubstitute_desktop::StartupBootstrapCause as Bootstrap;
        Code::Bootstrap(match issue {
            bootstrap::ChildBootstrapIssue::State => Bootstrap::State,
            bootstrap::ChildBootstrapIssue::Identity => Bootstrap::Identity,
            bootstrap::ChildBootstrapIssue::Transport => Bootstrap::Transport,
            bootstrap::ChildBootstrapIssue::Capture => Bootstrap::Capture,
            bootstrap::ChildBootstrapIssue::Configuration => Bootstrap::Configuration,
            bootstrap::ChildBootstrapIssue::Deadline => Bootstrap::Deadline,
        })
    } else if let Some(closed) =
        cause.downcast_ref::<sugarsubstitute_desktop::StartupCloseFailure>()
    {
        closed.code()
    } else {
        Code::direct(cause)
    }
}

/// Project the child's own expiry, output, panel-clock and wire-decoding owners.
fn child_cause_code(
    cause: &(dyn Error + 'static),
) -> Option<sugarsubstitute_desktop::StartupCauseCode> {
    use crate::production_output::OutputError;
    use sugarsubstitute_application::panel::PanelClockError;
    use sugarsubstitute_desktop::{
        StartupCauseCode as Code, StartupOutputCause as Output, StartupTimeCause as Time,
    };
    if cause.downcast_ref::<requests::RequestExpiry>().is_some() {
        return Some(Code::Request(sugarsubstitute_desktop::StartupRequestCause::Deadline));
    }
    if let Some(output) = cause.downcast_ref::<OutputError>() {
        return Some(match output {
            OutputError::Transcript(transcript) => Code::Transcript(*transcript),
            OutputError::Input(_) => Code::Output(Output::Input),
            OutputError::Clock(_) => Code::Output(Output::Clock),
            OutputError::Time(_) => Code::Output(Output::Time),
            OutputError::Claim(_) => Code::Output(Output::Claim),
            OutputError::Protocol(_) => Code::Output(Output::Protocol),
            OutputError::Identity => Code::Output(Output::Identity),
            OutputError::Order => Code::Output(Output::Order),
            OutputError::Cancelled => Code::Output(Output::Cancelled),
            OutputError::Deadline => Code::Output(Output::Deadline),
            OutputError::Poisoned => Code::Output(Output::Poisoned),
            OutputError::Terminal(kind) => Code::Output(Output::Terminal(*kind)),
        });
    }
    if let Some(clock) = cause.downcast_ref::<PanelClockError>() {
        return Some(Code::Time(match clock {
            PanelClockError::Regressed => Time::Regression,
            PanelClockError::DeadlineOverflow => Time::Overflow,
        }));
    }
    cause
        .downcast_ref::<sugarsubstitute_supervision::lifecycle_protocol::ProtocolError>()
        .map(|_| Code::Protocol)
}
