// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual consumer errors at their authored stage without changing completion delivery.
use super::StartupBackend;
use std::error::Error;
use sugarsubstitute_desktop::{StartupConsumeFailure, StartupConsumeStage};

/// Enrich only an actual returned error, preserving its original Box and direct typed category.
pub(super) fn observe<B: StartupBackend, T>(
    stage: StartupConsumeStage,
    result: Result<T, Box<dyn Error>>,
) -> Result<T, Box<dyn Error>> {
    result.map_err(|cause| {
        let code = B::startup_cause_code(cause.as_ref());
        StartupConsumeFailure::retain(stage, code, cause)
    })
}

/// Resolve all already consumed recipients and retain every original failure.
pub(super) fn delivery<B: StartupBackend, T>(
    render: Result<T, Box<dyn Error>>,
    child: Result<(), Box<dyn Error>>,
    canvas: Result<(), Box<dyn Error>>,
) -> Result<T, Box<dyn Error>> {
    let stage = if render.is_err() {
        StartupConsumeStage::RenderDelivery
    } else if child.is_err() {
        StartupConsumeStage::ChildDelivery
    } else {
        StartupConsumeStage::CanvasDelivery
    };
    super::delivery::resolve(super::delivery::resolve(render, child), canvas).map_err(|cause| {
        let code = B::startup_cause_code(super::delivery::first_cause(cause.as_ref()));
        StartupConsumeFailure::retain(stage, code, cause)
    })
}

#[cfg(all(test, windows, feature = "windows-native-host"))]
#[path = "consume_diagnostic_tests.rs"]
mod tests;
