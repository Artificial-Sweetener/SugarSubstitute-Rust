// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project only passive original capture and supported direct native/control causes.
use super::*;

impl ProductionChild {
    /// Inspect supported direct returned types; never sample capture or walk error sources.
    pub(in crate::production) fn startup_cause_code(
        cause: &(dyn Error + 'static),
    ) -> sugarsubstitute_desktop::StartupCauseCode {
        cause_code(cause)
    }
    /// Borrow the original snapshot and inspect supported direct error types without providers.
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

/// Inspect supported direct native/control types without walking arbitrary sources.
pub(super) fn cause_code(
    cause: &(dyn Error + 'static),
) -> sugarsubstitute_desktop::StartupCauseCode {
    use sugarsubstitute_desktop::{
        StartupCauseCode as Code, StartupNativeCause as Native, StartupRequestCause as Request,
    };
    if let Some(native) =
        cause.downcast_ref::<sugarsubstitute_execution::windows::NativeObservationError>()
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
