// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe preparation transitions and preserve closed first-cause categories.
use super::*;
impl ChildBootstrap {
    /// Observe actual preparation transitions after all admission/native guards are released.
    /// Only closed phase/input categories and booleans are logged; cause custody stays owned.
    pub(in crate::production::child) fn report_preparation(&mut self) {
        if let Some(observation) = self.changed_preparation() {
            tracing::info!(
                operation = "child_preparation_transition",
                channel_phase = ?observation.channel_phase, channel_terminal = ?observation.channel_terminal,
                capture_input = ?observation.capture_input,
                capture_admitted = observation.capture_admitted,
                capture = observation.capture,
                registered = observation.registered,
                configuration = observation.configuration,
                original_failure = ?observation.original_failure,
                capture_failure = ?observation.capture_failure,
                failure_retained = observation.failure_retained,
                "Child pre-UI preparation conditions changed"
            );
        }
    }
    /// Cache one closed observation to suppress identical polling diagnostics.
    pub(super) fn changed_preparation(&mut self) -> Option<PreparationObservation> {
        let observation = self.preparation_observation();
        if self.last_preparation == Some(observation) {
            return None;
        }
        self.last_preparation = Some(observation);
        Some(observation)
    }
    /// Borrow closed actual preparation predicates independently of fallible logging.
    pub(in crate::production::child) fn preparation_observation(&self) -> PreparationObservation {
        PreparationObservation {
            channel_phase: self.channel.handshake_phase(),
            channel_terminal: self.channel.terminal_kind(),
            capture_input: self.channel.capture_input(),
            capture_admitted: self.capture.admitted(),
            capture: self.capture.ready(),
            registered: self.channel.registered(),
            configuration: self.prepared.is_some() || self.delivered,
            original_failure: self.issue,
            capture_failure: self.capture_failure_kind(),
            failure_retained: self.failure_source().is_some(),
        }
    }
    /// Inspect only supported concrete first-cause types; never format their payload.
    pub(super) fn capture_failure_kind(&self) -> CaptureFailureKind {
        use super::super::capture::ChildCaptureError;
        use sugarsubstitute_contracts::execution::AdmissionError;
        use sugarsubstitute_execution::windows::NativeObservationError;
        if self.issue != Some(ChildBootstrapIssue::Capture) {
            return CaptureFailureKind::None;
        }
        let Some(cause) = self.failure_source() else {
            return CaptureFailureKind::None;
        };
        if let Some(cause) = cause.downcast_ref::<ChildCaptureError>() {
            return match cause {
                ChildCaptureError::State => CaptureFailureKind::State,
                ChildCaptureError::Identity => CaptureFailureKind::Identity,
            };
        }
        if cause.is::<AdmissionError>() {
            return CaptureFailureKind::Admission;
        }
        if cause.is::<NativeObservationError>() {
            return CaptureFailureKind::Native;
        }
        match cause.downcast_ref::<CaptureAttachFailure>() {
            Some(CaptureAttachFailure::Data(_)) => CaptureFailureKind::Data,
            Some(CaptureAttachFailure::Source(_)) => CaptureFailureKind::Source,
            Some(CaptureAttachFailure::Native(_)) => CaptureFailureKind::Native,
            Some(CaptureAttachFailure::TimedOut) => CaptureFailureKind::TimedOut,
            Some(CaptureAttachFailure::Cancelled) => CaptureFailureKind::Cancelled,
            Some(CaptureAttachFailure::Uncertain) => CaptureFailureKind::Uncertain,
            None => CaptureFailureKind::Other,
        }
    }
}
