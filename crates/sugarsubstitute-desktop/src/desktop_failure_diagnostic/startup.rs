// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enrich the existing first failure frame from actual observation and retained typed close facts.

use super::*;
use crate::{
    DesktopStartupObservation, StartupCaptureState, StartupCauseCode, StartupCloseFailure,
    StartupCloseStage,
};

/// Retain the exact content failure and actual observation tag without payload projection.
struct ObservationFailure {
    kind: &'static str,
    cause: Box<dyn Error>,
}
impl std::fmt::Debug for ObservationFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ObservationFailure")
    }
}
impl std::fmt::Display for ObservationFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Original startup observation failed")
    }
}
impl Error for ObservationFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

/// Copy only a closed kind from the actual native observation before transferring it.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn observation_kind(observation: &DesktopStartupObservation) -> &'static str {
    match observation {
        DesktopStartupObservation::NativeActivity { .. } => "NativeActivity",
        DesktopStartupObservation::UiChallengeProcessed(_) => "UiChallengeProcessed",
        DesktopStartupObservation::NativeRetired(_) => "NativeRetired",
        DesktopStartupObservation::SplashPresented(_) => "SplashPresented",
        DesktopStartupObservation::EnvironmentPrepared { .. } => "EnvironmentPrepared",
        DesktopStartupObservation::ControllerPrepared { .. } => "ControllerPrepared",
        DesktopStartupObservation::NavigationStarted(_) => "NavigationStarted",
        DesktopStartupObservation::FrontendReceipt { .. } => "FrontendReceipt",
        DesktopStartupObservation::SplashRetired(_) => "SplashRetired",
        DesktopStartupObservation::MainRevealed(_) => "MainRevealed",
        DesktopStartupObservation::MainReloadRequested { .. } => "MainReloadRequested",
        #[cfg(all(target_os = "windows", feature = "k1-native-qualification"))]
        DesktopStartupObservation::MainPreviewCaptured(_) => "MainPreviewCaptured",
        #[cfg(all(target_os = "windows", feature = "k1-native-qualification"))]
        DesktopStartupObservation::MainPreviewFailed { .. } => "MainPreviewFailed",
        DesktopStartupObservation::MainRetired(_) => "MainRetired",
        DesktopStartupObservation::MainAbsent(_) => "MainAbsent",
        DesktopStartupObservation::BackendRetryRequested { .. } => "BackendRetryRequested",
        DesktopStartupObservation::CloseRequested => "CloseRequested",
        DesktopStartupObservation::Failed { .. } => "Failed",
    }
}

impl super::super::DesktopError {
    /// Copy the closed kind before the real content observation consumes its owned payload.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn startup_observation_kind(
        observation: &DesktopStartupObservation,
    ) -> &'static str {
        observation_kind(observation)
    }
    /// Preserve the actual failed content observation with its unchanged original Box.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn observed_startup(kind: &'static str, cause: Box<dyn Error>) -> Self {
        Self::new("observe_startup", ObservationFailure { kind, cause })
    }
}

/// Emit an enriched existing generic frame only for a concrete authored observation failure.
/// All data comes from retained typed fields; no arbitrary formatter or source callback runs.
pub(super) fn emit(operation: &'static str, cause: &(dyn Error + 'static)) -> bool {
    if operation == "poll_frame" {
        if let Some(consumed) = cause.downcast_ref::<crate::StartupConsumeFailure>() {
            let actual = classify(operation, cause);
            tracing::warn!(operation="desktop_failure_transition",boundary=?actual.boundary,kind=?actual.kind,
                consume_stage=?consumed.stage(),category=%consumed.code(),"First owned desktop failure retained");
            return true;
        }
        return false;
    }
    if operation != "observe_startup" {
        return false;
    }
    let Some(observed) = cause.downcast_ref::<ObservationFailure>() else { return false };
    let (stage, capture, code) = observed.cause.downcast_ref::<StartupCloseFailure>().map_or(
        (
            StartupCloseStage::None,
            StartupCaptureState::Unobserved,
            StartupCauseCode::direct(observed.cause.as_ref()),
        ),
        |failure| (failure.stage(), failure.capture(), failure.code()),
    );
    let actual = classify(operation, cause);
    tracing::warn!(operation = "desktop_failure_transition", boundary = ?actual.boundary,
        kind = ?actual.kind, observation = %observed.kind, close_stage = ?stage,
        capture_snapshot = ?capture, code = %code,
        "First owned desktop failure retained");
    true
}
