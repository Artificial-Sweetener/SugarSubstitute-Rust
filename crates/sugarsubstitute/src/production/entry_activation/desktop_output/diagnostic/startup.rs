// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode closed startup failure enrichment inside the existing first-failure reservation.

use sugarsubstitute_desktop::{
    StartupCaptureState as Capture, StartupCauseCode as Code, StartupCloseStage as Stage,
    StartupNativeCause as Native, StartupRequestCause as Request,
};

/// Inert actual observation and first-close facts; Run belongs to the authenticated output owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct StartupFailureDetail {
    pub(in super::super) observation: &'static str,
    pub(in super::super) stage: Stage,
    pub(in super::super) capture: Capture,
    pub(in super::super) code: Code,
}

const OBSERVATIONS: &[&str] = &[
    "NativeActivity",
    "UiChallengeProcessed",
    "NativeRetired",
    "SplashPresented",
    "EnvironmentPrepared",
    "ControllerPrepared",
    "NavigationStarted",
    "FrontendReceipt",
    "SplashRetired",
    "MainRevealed",
    #[cfg(feature = "k1-native-qualification")]
    "MainPreviewCaptured",
    #[cfg(feature = "k1-native-qualification")]
    "MainPreviewFailed",
    "MainRetired",
    "MainAbsent",
    "CloseRequested",
    "Failed",
];

/// Require complete authored fields, canonical numeric causes and no raw extensions.
pub(super) fn parse(
    mut fields: std::str::SplitAsciiWhitespace<'_>,
) -> Option<StartupFailureDetail> {
    let observed = fields.next()?.strip_prefix("observation=")?;
    let observation = OBSERVATIONS.iter().copied().find(|known| *known == observed)?;
    let stage = match fields.next()?.strip_prefix("close_stage=")? {
        "None" => Stage::None,
        "CaptureObservation" => Stage::CaptureObservation,
        "ShutdownBudget" => Stage::ShutdownBudget,
        "ExitIntent" => Stage::ExitIntent,
        "PolicyClose" => Stage::PolicyClose,
        "PolicyApply" => Stage::PolicyApply,
        "RenderRetirement" => Stage::RenderRetirement,
        "DeadlineRefresh" => Stage::DeadlineRefresh,
        _ => return None,
    };
    let capture = match fields.next()?.strip_prefix("capture_snapshot=")? {
        "Unobserved" => Capture::Unobserved,
        "Absent" => Capture::Absent,
        "Dormant" => Capture::Dormant,
        "Requested" => Capture::Requested,
        "Latched" => Capture::Latched,
        "CounterFailed" => Capture::CounterFailed,
        "ClockUnavailable" => Capture::ClockUnavailable,
        _ => return None,
    };
    let code = code(fields.next()?.strip_prefix("code=")?)?;
    if fields.next().is_some() {
        return None;
    }
    Some(StartupFailureDetail { observation, stage, capture, code })
}

fn native(value: &str) -> Option<Native> {
    Some(match value {
        "CaptureClock" => Native::CaptureClock,
        "CaptureBudget" => Native::CaptureBudget,
        "CaptureOwnerUnavailable" => Native::CaptureOwnerUnavailable,
        "CaptureCounterArithmetic" => Native::CaptureCounterArithmetic,
        "CounterQuery" => Native::CounterQuery,
        "CounterFrequency" => Native::CounterFrequency,
        "CounterPositive" => Native::CounterPositive,
        "CounterClock" => Native::CounterClock,
        "CounterConversion" => Native::CounterConversion,
        "CounterArithmetic" => Native::CounterArithmetic,
        "CounterShape" => Native::CounterShape,
        "Other" => Native::Other,
        "OwnerPanic" => Native::OwnerPanic,
        _ => return None,
    })
}

fn signed(value: &str) -> Option<i32> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || (digits.starts_with('0') && digits.len() > 1)
        || value == "-0"
    {
        return None;
    }
    value.parse().ok()
}

pub(super) fn code(value: &str) -> Option<Code> {
    if let Some(value) = value.strip_prefix("Bootstrap(").and_then(|v| v.strip_suffix(')')) {
        use sugarsubstitute_desktop::StartupBootstrapCause as Bootstrap;
        return Some(Code::Bootstrap(match value {
            "State" => Bootstrap::State,
            "Identity" => Bootstrap::Identity,
            "Transport" => Bootstrap::Transport,
            "Capture" => Bootstrap::Capture,
            "Configuration" => Bootstrap::Configuration,
            "Deadline" => Bootstrap::Deadline,
            _ => return None,
        }));
    }
    Some(match value {
        "Opaque" => Code::Opaque,
        "TimedOut" => Code::TimedOut,
        "Disconnected" => Code::Disconnected,
        "Denied" => Code::Denied,
        "InvalidInput" => Code::InvalidInput,
        "Io" => Code::Io,
        _ => {
            if let Some(raw) = value.strip_prefix("Os(").and_then(|s| s.strip_suffix(')')) {
                return signed(raw).map(Code::Os);
            }
            if let Some(raw) = value.strip_prefix("Hresult(").and_then(|s| s.strip_suffix(')')) {
                return signed(raw).map(Code::Hresult);
            }
            if let Some(raw) = value.strip_prefix("Native(").and_then(|s| s.strip_suffix(')')) {
                return native(raw).map(|stage| Code::Native(stage, None));
            }
            if let Some(raw) = value.strip_prefix("NativeOs(").and_then(|s| s.strip_suffix(')')) {
                let (stage, code) = raw.split_once(',')?;
                let parsed: u32 = code.parse().ok()?;
                if code.is_empty()
                    || !code.bytes().all(|byte| byte.is_ascii_digit())
                    || (code.starts_with('0') && code.len() > 1)
                {
                    return None;
                }
                return Some(Code::Native(native(stage)?, Some(parsed)));
            }
            if let Some(raw) = value.strip_prefix("Request(").and_then(|s| s.strip_suffix(')')) {
                return Some(Code::Request(match raw {
                    "Registration" => Request::Registration,
                    "Closed" => Request::Closed,
                    "Capacity" => Request::Capacity,
                    "Exhausted" => Request::Exhausted,
                    "Identity" => Request::Identity,
                    "Frequency" => Request::Frequency,
                    "Order" => Request::Order,
                    "Deadline" => Request::Deadline,
                    _ => return None,
                }));
            }
            return None;
        }
    })
}

#[cfg(test)]
#[path = "startup_tests.rs"]
mod tests;
