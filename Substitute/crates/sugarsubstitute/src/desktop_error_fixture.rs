// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Obtain a real adapter validation error before any native surface effect.
use std::{error::Error, num::NonZeroUsize, sync::Arc, time::Duration};
use sugarsubstitute_contracts::{
    execution::CompletionWake,
    presentation::{FrameShape, PreparedFrame},
};
use sugarsubstitute_desktop::{
    AccessibleControl, DesktopError, DesktopOptions, HitTarget, KeyAction, Pointer, PointerAction,
    WindowContent,
};

/// The existing pure dimension guard returns the actual opaque DesktopError.
/// # Errors
/// Unexpected acceptance or another adapter cause rejects the fixture; no native privileges exist.
pub(crate) fn native_failure() -> Result<DesktopError, Box<dyn Error>> {
    let options = DesktopOptions {
        title: String::new(),
        logical_width: 0,
        logical_height: 1,
        dark: true,
        duration: Some(Duration::ZERO),
        audit_frames: false,
        audit_timings: false,
    };
    let cause = sugarsubstitute_desktop::run(options, UnusedContent)
        .err()
        .ok_or("Invalid dimensions accepted")?;
    if !cause.to_string().contains("client dimensions must be positive") {
        return Err("Pure adapter validation boundary was bypassed".into());
    }
    Ok(cause)
}
struct UnusedContent;
fn unused<T>() -> Result<T, Box<dyn Error>> {
    Err("Content must remain untouched before dimension validation".into())
}
impl WindowContent for UnusedContent {
    fn start(&mut self, _: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        unused()
    }
    fn request_frame(&mut self, _: FrameShape, _: Duration) -> Result<(), Box<dyn Error>> {
        unused()
    }
    fn poll_frame(&mut self, _: NonZeroUsize) -> Result<Option<PreparedFrame>, Box<dyn Error>> {
        unused()
    }
    fn set_visible(&mut self, _: bool) -> Result<(), Box<dyn Error>> {
        unused()
    }
    fn request_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        unused()
    }
    fn poll_shutdown(&mut self) -> Result<bool, Box<dyn Error>> {
        unused()
    }
    fn finish_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        unused()
    }
    fn hit_test(&self, _: Pointer) -> HitTarget {
        HitTarget::Client
    }
    fn pointer(&mut self, _: PointerAction, _: Pointer) -> bool {
        false
    }
    fn key(&mut self, _: KeyAction) -> bool {
        false
    }
    fn animating(&self) -> bool {
        false
    }
    fn console(&self) -> Option<sugarsubstitute_desktop::ConsoleView<'_>> {
        None
    }
    fn accessible_controls(&self) -> &[AccessibleControl] {
        &[]
    }
    fn activate(&mut self, _: u16) -> bool {
        false
    }
}
