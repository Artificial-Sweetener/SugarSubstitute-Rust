// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Configure the installed frontend's owned desktop frame through presentation protocol one.

use super::*;
use crate::main_readiness_types::ProbeTarget;
use serde::Deserialize;
use wry::WebViewExtWindows;

/// A closed request from a read-only setup sample; it cannot itself acknowledge presentation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Needed {
    configure_desktop_frame: u8,
}
impl Needed {
    /// Distinguish the exact request envelope from incomplete or final readiness receipts.
    pub(super) fn decode(text: &str) -> bool {
        serde_json::from_str::<Self>(text).is_ok_and(|value| value.configure_desktop_frame == 1)
    }
}

/// Protocol-one values are logical CSS pixels; the receipt grants no native capabilities.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct Receipt {
    schema_version: u8,
    caption_width: u16,
    titlebar_height: u16,
    dark_theme: bool,
    mounted: bool,
}
impl Receipt {
    /// Reject version, geometry or native-theme mismatches before releasing mutation custody.
    pub(super) fn validate(&self, dark: bool) -> Result<(), DesktopError> {
        if self.schema_version != 1
            || self.caption_width != 138
            || self.titlebar_height != 34
            || self.dark_theme != dark
        {
            return Err(DesktopError::message(
                "configure_desktop_frame",
                "SugarCubes desktop frame protocol or geometry differs",
            ));
        }
        Ok(())
    }
}

/// Retain the genuine original native operation while its completion crosses the bounded queue.
pub(super) struct Completion {
    pub(super) target: ProbeTarget,
    pub(super) receipt: Receipt,
}

impl MainActor {
    /// Submit one finite mutation in the original attributed top-level execution context.
    pub(super) fn configure_frame(&self, target: ProbeTarget) -> Result<(), DesktopError> {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings9;
        use windows::core::{BOOL, Interface};
        let core = self.view()?.webview();
        // SAFETY: This actor retains the original UI-affine CoreWebView2 throughout the getter.
        let settings = unsafe { core.Settings() }
            .map_err(|cause| DesktopError::new("desktop_frame_native_regions", cause))?;
        let settings: ICoreWebView2Settings9 = settings
            .cast()
            .map_err(|cause| DesktopError::new("desktop_frame_native_regions", cause))?;
        let mut enabled = BOOL::default();
        // SAFETY: The retained UI-affine settings copy a scalar into a live local output.
        unsafe { settings.IsNonClientRegionSupportEnabled(&raw mut enabled) }
            .map_err(|cause| DesktopError::new("desktop_frame_native_regions", cause))?;
        if !enabled.as_bool() {
            return Err(DesktopError::message(
                "desktop_frame_native_regions",
                "native frame regions are disabled",
            ));
        }
        let parameters = parameters(target, self.request()?.settings().dark)?;
        self.receivers()?.call(Command::FrameConfiguration(target), "Runtime.evaluate", &parameters)
    }

    /// Accept configuration separately from usability; only a fresh sample may start the barrier.
    pub(super) fn frame_configured(&mut self, value: Completion) -> Result<(), DesktopError> {
        value.receipt.validate(self.request()?.settings().dark)?;
        self.readiness()?
            .complete_frame_configuration(value.target, std::time::Instant::now())
            .map_err(readiness_error)?;
        tracing::debug!(operation="configure_desktop_frame", identity=?self.identity(),
            mounted=value.receipt.mounted, "Installed SugarCubes desktop presentation configured");
        Ok(())
    }
}

/// Fill the client area in physical pixels; web-owned tabs share the native caption's first row.
pub(super) fn browser_bounds(width: u32, height: u32) -> wry::Rect {
    wry::Rect {
        position: wry::dpi::PhysicalPosition::new(0, 0).into(),
        size: wry::dpi::PhysicalSize::new(width.max(1), height.max(1)).into(),
    }
}

/// Invoke the supported presentation owner; no DOM selectors or copied frontend controls live here.
fn parameters(target: ProbeTarget, dark: bool) -> Result<String, DesktopError> {
    let expression = format!(
        "(()=>{{const api=window.SugarCubes;if(typeof api?.configureDesktopFrame!=='function')throw new Error('SugarCubes desktop frame protocol 1 is unavailable.');return JSON.stringify(api.configureDesktopFrame({{schemaVersion:1,captionWidth:138,titlebarHeight:34,darkTheme:{dark}}}));}})()"
    );
    serde_json::to_string(&serde_json::json!({
        "expression":expression,
        "contextId":target.context,
        "returnByValue":true,
    }))
    .map_err(|cause| DesktopError::new("encode_desktop_frame", cause))
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
