// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Closed capture failure facts preserve native codes without relaying error text.
#[cfg(any(windows, test))]
use crate::DesktopError;
use crate::MainIdentity;
#[cfg(windows)]
use std::error::Error;

/// The actual owner boundary returning the original capture failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativePhase {
    /// Failure at the admission boundary.
    Admission,
    /// Failure at the seal boundary.
    Seal,
    /// Failure at the storage boundary.
    Storage,
    /// Failure at the issue boundary.
    Issue,
    /// Failure at the revalidate boundary.
    Revalidate,
    /// Failure at the publication boundary.
    Publication,
    /// Failure at the callback boundary.
    Callback,
    /// Failure at the stream boundary.
    Stream,
}
impl NativePhase {
    /// Decode one known phase; records confer no capture authority.
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "Admission" => Some(Self::Admission),
            "Seal" => Some(Self::Seal),
            "Storage" => Some(Self::Storage),
            "Issue" => Some(Self::Issue),
            "Revalidate" => Some(Self::Revalidate),
            "Publication" => Some(Self::Publication),
            "Callback" => Some(Self::Callback),
            "Stream" => Some(Self::Stream),
            _ => None,
        }
    }
}
/// Fixed native failure facts; original error ownership stays with MainActor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeFailure {
    /// Actual returning boundary, or the original callback/stream error check.
    pub phase: NativePhase,
    /// Closed adapter operation, with unknown adapter helpers represented as Other.
    pub operation: &'static str,
    /// Original failing Windows HRESULT, never an invented code for a message error.
    pub hresult: Option<i32>,
}
/// Decode an exact known adapter operation without retaining arbitrary external text.
pub fn native_operation(token: &str) -> Option<&'static str> {
    match token {
        "capture_main_preview" => Some("capture_main_preview"),
        "claim_preview_callback" => Some("claim_preview_callback"),
        "observe_preview_content" => Some("observe_preview_content"),
        "observe_preview_browser" => Some("observe_preview_browser"),
        "observe_preview_bounds" => Some("observe_preview_bounds"),
        "observe_preview_dpi" => Some("observe_preview_dpi"),
        "observe_preview_versions" => Some("observe_preview_versions"),
        "observe_preview_cutoff" => Some("observe_preview_cutoff"),
        "observe_preview_desktop" => Some("observe_preview_desktop"),
        "open_preview_input_desktop" => Some("open_preview_input_desktop"),
        "observe_preview_station" => Some("observe_preview_station"),
        "observe_preview_thread_desktop" => Some("observe_preview_thread_desktop"),
        "observe_preview_desktop_name" => Some("observe_preview_desktop_name"),
        "close_preview_input_desktop" => Some("close_preview_input_desktop"),
        "complete_main_preview" => Some("complete_main_preview"),
        "complete_preview_stream" => Some("complete_preview_stream"),
        "take_main_preview" => Some("take_main_preview"),
        "main_request" => Some("main_request"),
        "main_window" => Some("main_window"),
        "main_events" => Some("main_events"),
        "main_view" => Some("main_view"),
        "main_readiness" => Some("main_readiness"),
        "main_receivers" => Some("main_receivers"),
        "main_frontend_effect" => Some("main_frontend_effect"),
        "observe_main_visibility" => Some("observe_main_visibility"),
        "query_performance_frequency" => Some("query_performance_frequency"),
        "query_performance_counter" => Some("query_performance_counter"),
        "Other" => Some("Other"),
        _ => None,
    }
}
impl NativeFailure {
    /// Observe only bounded original error sources, without allocating or changing custody.
    #[cfg(any(windows, test))]
    pub(crate) fn from_error(phase: NativePhase, error: &DesktopError) -> Self {
        let operation = native_operation(error.qualification_operation()).unwrap_or("Other");
        let hresult = original_hresult(error);
        let phase = match operation {
            "complete_main_preview" if hresult.is_some() => NativePhase::Callback,
            "complete_preview_stream" | "take_main_preview" => NativePhase::Stream,
            _ => phase,
        };
        Self { phase, operation, hresult }
    }
}
/// Emit one actual first failure before transferring its unchanged error to the Host.
pub fn native_failed(identity: MainIdentity, detail: NativeFailure) {
    tracing::warn!(target: "sugarsubstitute_desktop::qualification_trace",
        operation="child_native_qualification", schema=1, stage="NativePreviewFailed",
        main=true, launch=identity.launch.get(), navigation=identity.navigation.get(),
        phase=?detail.phase, native_operation=detail.operation,
        has_hresult=detail.hresult.is_some(), hresult=detail.hresult.unwrap_or(0));
}

#[cfg(windows)]
fn original_hresult(error: &DesktopError) -> Option<i32> {
    let mut source = error.source();
    for _ in 0..16 {
        let cause = source?;
        if let Some(native) = cause.downcast_ref::<windows::core::Error>() {
            return native.code().is_err().then_some(native.code().0);
        }
        source = cause.source();
    }
    None
}
#[cfg(all(not(windows), test))]
fn original_hresult(_: &DesktopError) -> Option<i32> {
    None
}
#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
