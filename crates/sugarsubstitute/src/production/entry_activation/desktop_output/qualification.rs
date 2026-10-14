// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Relay only typed output matching the already authoritative parent identity.
use super::diagnostic::qualification::{Detail, Qualification};
use sugarsubstitute_contracts::{lifecycle::RunKey, startup::MainIdentity};
use sugarsubstitute_desktop::qualification_trace::Progress;

/// Called only after the owned batch run check, independently of native protocol routing.
pub(super) fn relay(run: RunKey, current: Option<MainIdentity>, observed: Qualification) -> bool {
    let identity = if observed.launch == 0 && observed.navigation == 0 {
        None
    } else {
        let Some(current) = current else {
            return false;
        };
        if current.run != run
            || current.launch.get() != observed.launch
            || current.navigation.get() != observed.navigation
        {
            return false;
        }
        Some(current)
    };
    match observed.detail {
        Detail::Progress(stage) => {
            let Some(identity) = identity else {
                return false;
            };
            let operation = match stage {
                Progress::EnvironmentDispatched
                | Progress::BrowserDispatched
                | Progress::RevealDispatched
                | Progress::PreviewDispatched => "main_command_dispatched",
                Progress::BrowserAuthorized => "main_browser_authorized",
                Progress::NavigationStarted => "main_navigation_started",
                Progress::NavigationCompleted => "main_navigation_completed",
                Progress::FrontendReady | Progress::FrontendFailed => "main_frontend_receipt",
                Progress::MainRevealed => "main_revealed",
                Progress::PreviewRequested => "main_preview_requested",
            };
            tracing::info!(operation, run=?run, identity=?identity, stage=?stage,
                "Observed closed owned Desktop transition");
        }
        Detail::Failed(cause) => {
            tracing::warn!(operation="main_preview_qualification_failed", run=?run,
                identity=?identity, cause_kind=?cause, "Observed closed original preview failure");
        }
        Detail::NativeFailed(detail) => {
            let Some(identity) = identity else {
                return false;
            };
            tracing::warn!(operation="main_preview_native_failed", run=?run, identity=?identity,
                phase=?detail.phase, native_operation=detail.operation,
                has_hresult=detail.hresult.is_some(), hresult=detail.hresult.unwrap_or(0),
                "Observed original native capture failure");
        }
        Detail::Persisted(proof) => {
            let Some(identity) = identity else {
                return false;
            };
            tracing::info!(operation="main_preview_persisted", run=?run, identity=?identity,
                png_bytes=proof.bytes, png_sha256=%Digest(&proof.digest),
                native_navigation=proof.native_navigation, viewport_width=proof.width,
                viewport_height=proof.height, dpi=proof.dpi,
                "Observed sealed activated PNG and checked artifact retirement");
        }
        Detail::Finished => {
            tracing::info!(operation="production_desktop_retired", run=?run,
                "Observed successful actual Desktop shutdown return");
        }
    }
    true
}
struct Digest<'a>(&'a [u8; 32]);
impl std::fmt::Display for Digest<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "qualification_tests.rs"]
mod tests;
