// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Measure and steer the embedded browser's memory for the application memory budget.
//!
//! Both operations run on the browser's UI apartment and return without waiting.

use super::*;
use crate::MainIdentity;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    ICoreWebView2, ICoreWebView2_2, ICoreWebView2_19, ICoreWebView2Environment8,
};
use windows::core::Interface;

impl MainActor {
    /// Report the identities of every process the current browser environment runs.
    ///
    /// # Errors
    /// Rejects a foreign or unrevealed identity and preserves WebView2 query failures.
    pub(crate) fn measure_browser(
        &self,
        identity: MainIdentity,
    ) -> Result<Option<DesktopStartupObservation>, DesktopError> {
        let core = self.browser_core(identity)?;
        let processes = browser_processes(core)
            .map_err(|cause| DesktopError::new("measure_browser_processes", cause))?;
        Ok(Some(DesktopStartupObservation::BrowserProcesses { identity, processes }))
    }

    /// Ask the browser to trim its memory (`low`) or return to its normal working set.
    ///
    /// # Errors
    /// Rejects a foreign or unrevealed identity and preserves WebView2 failures.
    pub(crate) fn set_browser_memory_target(
        &self,
        identity: MainIdentity,
        low: bool,
    ) -> Result<(), DesktopError> {
        let core = self.browser_core(identity)?;
        let level = if low {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
        } else {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
        };
        let view: ICoreWebView2_19 =
            core.cast().map_err(|cause| DesktopError::new("browser_memory_target", cause))?;
        // SAFETY: the interface was obtained from the live browser on its own UI apartment.
        unsafe { view.SetMemoryUsageTargetLevel(level) }
            .map_err(|cause| DesktopError::new("browser_memory_target", cause))?;
        tracing::info!(operation = "browser_memory_target", low, "Browser memory target changed");
        Ok(())
    }

    fn browser_core(&self, identity: MainIdentity) -> Result<&ICoreWebView2, DesktopError> {
        self.admission.frontend(identity)?;
        self.panel_bridge
            .as_ref()
            .map(panel::bridge::Bridge::core)
            .ok_or_else(|| DesktopError::message("browser_memory", "the browser is unavailable"))
    }
}

fn browser_processes(core: &ICoreWebView2) -> windows::core::Result<Vec<u32>> {
    let core: ICoreWebView2_2 = core.cast()?;
    // SAFETY: every call below targets live COM objects on their own UI apartment, and the
    // out-parameters are writable locals of the declared types.
    unsafe {
        let environment: ICoreWebView2Environment8 = core.Environment()?.cast()?;
        let infos = environment.GetProcessInfos()?;
        let mut count = 0_u32;
        infos.Count(&raw mut count)?;
        let mut processes = Vec::with_capacity(count as usize);
        for index in 0..count {
            let mut process = 0_i32;
            infos.GetValueAtIndex(index)?.ProcessId(&raw mut process)?;
            if let Ok(process) = u32::try_from(process) {
                processes.push(process);
            }
        }
        Ok(processes)
    }
}
