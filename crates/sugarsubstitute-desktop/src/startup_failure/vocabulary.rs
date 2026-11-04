// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the closed tokens of the first desktop failure frame, which the parent decodes.
//!
//! The desktop emits each fact through its `Debug` or `Display` form; the parent's output
//! relay decodes the same text with these parsers, so a variant added here is accepted there
//! without a second vocabulary. Every parser accepts exactly one canonical token, allocates
//! nothing and retains no input.
use super::{
    StartupBootstrapCause, StartupCaptureState, StartupCauseCode, StartupCloseStage,
    StartupNativeCause, StartupRequestCause,
};
use crate::StartupConsumeStage;
use sugarsubstitute_contracts::transcript::TranscriptError;

/// Derive the token list and exact parser of a fieldless enum from one variant listing.
///
/// The generated `token` match is exhaustive, so a variant missing from the listing fails to
/// compile instead of being silently refused by the parent.
macro_rules! closed_tokens {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        impl $name {
            /// Every variant, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The variant's diagnostic token, identical to its `Debug` form.
            pub const fn token(self) -> &'static str {
                match self { $(Self::$variant => stringify!($variant)),+ }
            }

            /// Decode one exact token; unknown, extended or differently cased text is refused.
            pub fn parse(token: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|variant| variant.token() == token)
            }
        }
    };
}

/// The startup observation the desktop was delivering when its content failed.
///
/// This is a closed label of [`crate::DesktopStartupObservation`]; it carries none of the
/// observation's payload and grants no authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupObservationKind {
    /// Native window activity facts.
    NativeActivity,
    /// A processed UI challenge.
    UiChallengeProcessed,
    /// Native retirement of the run.
    NativeRetired,
    /// The splash presented its first frame.
    SplashPresented,
    /// The browser environment was prepared.
    EnvironmentPrepared,
    /// The browser controller was prepared.
    ControllerPrepared,
    /// A main navigation started.
    NavigationStarted,
    /// The frontend readiness receipt.
    FrontendReceipt,
    /// The splash retired.
    SplashRetired,
    /// The main window was revealed.
    MainRevealed,
    /// A panel protocol message.
    PanelMessage,
    /// A memory protocol message.
    MemoryMessage,
    /// Browser process facts.
    BrowserProcesses,
    /// A canvas surface protocol message.
    SurfaceMessage,
    /// Routed canvas input.
    CanvasInput,
    /// Canvas display facts.
    CanvasDisplay,
    /// A native canvas protocol message.
    CanvasNativeMessage,
    /// The page asked for a main reload.
    MainReloadRequested,
    /// A qualification preview capture.
    MainPreviewCaptured,
    /// A failed qualification preview capture.
    MainPreviewFailed,
    /// The main window retired.
    MainRetired,
    /// The main window is absent.
    MainAbsent,
    /// The user asked to retry the backend.
    BackendRetryRequested,
    /// The user asked to close.
    CloseRequested,
    /// An owned native failure.
    Failed,
}

/// The closed reason a native canvas message could not be bound for delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupCanvasMessageCause {
    /// Its retained storage did not cover the message.
    Storage,
    /// It addressed another main navigation.
    ForeignNavigation,
    /// The navigation could not be represented as a panel session.
    Identity,
}

closed_tokens!(StartupObservationKind {
    NativeActivity,
    UiChallengeProcessed,
    NativeRetired,
    SplashPresented,
    EnvironmentPrepared,
    ControllerPrepared,
    NavigationStarted,
    FrontendReceipt,
    SplashRetired,
    MainRevealed,
    PanelMessage,
    MemoryMessage,
    BrowserProcesses,
    SurfaceMessage,
    CanvasInput,
    CanvasDisplay,
    CanvasNativeMessage,
    MainReloadRequested,
    MainPreviewCaptured,
    MainPreviewFailed,
    MainRetired,
    MainAbsent,
    BackendRetryRequested,
    CloseRequested,
    Failed,
});
closed_tokens!(StartupCanvasMessageCause { Storage, ForeignNavigation, Identity });
closed_tokens!(StartupCloseStage {
    None,
    CaptureObservation,
    ShutdownBudget,
    ExitIntent,
    PolicyClose,
    PolicyApply,
    RenderRetirement,
    CanvasRetirement,
    DeadlineRefresh,
});
closed_tokens!(StartupCaptureState {
    Unobserved,
    Absent,
    Dormant,
    Requested,
    Latched,
    CounterFailed,
    ClockUnavailable,
});
closed_tokens!(StartupNativeCause {
    CaptureClock,
    CaptureBudget,
    CaptureOwnerUnavailable,
    CaptureCounterArithmetic,
    CounterQuery,
    CounterFrequency,
    CounterPositive,
    CounterClock,
    CounterConversion,
    CounterArithmetic,
    CounterShape,
    Other,
    OwnerPanic,
});
closed_tokens!(StartupRequestCause {
    Registration,
    Closed,
    Capacity,
    Exhausted,
    Identity,
    Frequency,
    Order,
    Deadline,
});
closed_tokens!(StartupBootstrapCause {
    State,
    Identity,
    Transport,
    Capture,
    Configuration,
    Deadline,
});
closed_tokens!(StartupConsumeStage {
    Retry,
    ExecutionDrain,
    RenderDelivery,
    ChildDelivery,
    CanvasDelivery,
    StartupEvent,
    PolicyObservation,
    EffectRouting,
    DeadlineRefresh,
});

impl std::fmt::Display for StartupObservationKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.token())
    }
}

impl StartupCauseCode {
    /// Decode one exact `Display` form of a cause code.
    ///
    /// Native integers use signed decimal without plus signs, leading zeroes or negative zero;
    /// unknown categories and extensions are refused.
    pub fn parse(value: &str) -> Option<Self> {
        let wrapped = |prefix: &str| value.strip_prefix(prefix).and_then(|v| v.strip_suffix(')'));
        if let Some(inner) = wrapped("Transcript(") {
            return transcript(inner).map(Self::Transcript);
        }
        if let Some(inner) = wrapped("Bootstrap(") {
            return StartupBootstrapCause::parse(inner).map(Self::Bootstrap);
        }
        if let Some(inner) = wrapped("Request(") {
            return StartupRequestCause::parse(inner).map(Self::Request);
        }
        if let Some(inner) = wrapped("CanvasMessage(") {
            return StartupCanvasMessageCause::parse(inner).map(Self::CanvasMessage);
        }
        if let Some(inner) = wrapped("Os(") {
            return signed(inner).map(Self::Os);
        }
        if let Some(inner) = wrapped("Hresult(") {
            return signed(inner).map(Self::Hresult);
        }
        if let Some(inner) = wrapped("Native(") {
            return StartupNativeCause::parse(inner).map(|stage| Self::Native(stage, None));
        }
        if let Some(inner) = wrapped("NativeOs(") {
            let (stage, code) = inner.split_once(',')?;
            return Some(Self::Native(StartupNativeCause::parse(stage)?, Some(unsigned(code)?)));
        }
        Some(match value {
            "Opaque" => Self::Opaque,
            "TimedOut" => Self::TimedOut,
            "Disconnected" => Self::Disconnected,
            "Denied" => Self::Denied,
            "InvalidInput" => Self::InvalidInput,
            "Io" => Self::Io,
            _ => return None,
        })
    }
}

/// Decode a transcript rejection by its `Debug` form.
fn transcript(value: &str) -> Option<TranscriptError> {
    Some(match value {
        "ZeroIdentity" => TranscriptError::ZeroIdentity,
        "Bound" => TranscriptError::Bound,
        "EmptyGap" => TranscriptError::EmptyGap,
        "InvalidStyle" => TranscriptError::InvalidStyle,
        "ForeignResource" => TranscriptError::ForeignResource,
        "Sequence" => TranscriptError::Sequence,
        "StreamEnded" => TranscriptError::StreamEnded,
        "Exhausted" => TranscriptError::Exhausted,
        "SnapshotCapacity" => TranscriptError::SnapshotCapacity,
        _ => return None,
    })
}

/// Decode canonical signed decimal: no plus sign, leading zeroes or negative zero.
fn signed(value: &str) -> Option<i32> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if !canonical_digits(digits) || value == "-0" {
        return None;
    }
    value.parse().ok()
}

/// Decode canonical unsigned decimal without a sign or leading zeroes.
fn unsigned(value: &str) -> Option<u32> {
    canonical_digits(value).then(|| value.parse().ok()).flatten()
}

fn canonical_digits(digits: &str) -> bool {
    !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && !(digits.starts_with('0') && digits.len() > 1)
}

#[cfg(test)]
#[path = "vocabulary_tests.rs"]
mod tests;
