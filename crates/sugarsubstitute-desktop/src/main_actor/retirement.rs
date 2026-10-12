// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire receivers before controllers and preserve parent ownership across failed native cleanup.

use super::*;
use crate::main_readiness_types::ReadinessFault;
use tao::platform::windows::WindowExtWindows;
use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::DestroyWindow};

impl MainActor {
    /// Close all admission before any native teardown effect, without inferring settlement.
    pub(crate) fn close(&mut self, identity: crate::MainIdentity) -> Result<(), DesktopError> {
        if identity != self.identity() {
            return Err(DesktopError::message("retire_main", "main identity is stale or foreign"));
        }
        self.admission.close();
        if let Some(readiness) = &mut self.readiness {
            readiness.invalidate(ReadinessFault::Cancelled);
        }
        if let Some(events) = &self.events {
            events.close_admission();
        }
        if let Some(constructor) = &mut self.constructor {
            constructor.cancel();
        }
        Ok(())
    }

    /// A true result requires every native receiver, creation callback, controller and HWND retired.
    pub(super) fn retire_step(
        &mut self,
    ) -> Result<Option<DesktopStartupObservation>, DesktopError> {
        if let Some(receivers) = &mut self.receivers {
            if receivers.retire_step()? {
                self.receivers = None;
            }
            return Ok(None);
        }
        if let Some(constructor) = &mut self.constructor {
            if constructor
                .checked_retire()
                .map_err(|cause| DesktopError::new("retire_main_constructor", cause))?
            {
                self.constructor = None;
            }
            return Ok(None);
        }
        if let Some(partial) = &mut self.partial {
            if partial
                .checked_retire()
                .map_err(|cause| DesktopError::new("retire_partial_main", cause))?
            {
                self.partial = None;
            }
            return Ok(None);
        }
        if let Some(view) = &mut self.view {
            if view
                .checked_retire()
                .map_err(|cause| DesktopError::new("retire_main_view", cause))?
            {
                self.view = None;
            }
            return Ok(None);
        }
        if let Some(controls) = &mut self.controls {
            controls.shutdown()?;
            if controls.retired() {
                self.controls = None;
            }
            return Ok(None);
        }
        if let Some(presenter) = &mut self.presenter {
            presenter.shutdown()?;
            self.presenter = None;
            return Ok(None);
        }
        if let Some(window) = &self.window {
            let hwnd = HWND(window.hwnd() as *mut std::ffi::c_void);
            // SAFETY: All child interfaces and callback registrations have retired;
            // this creating thread retains the exact parent until actual destruction succeeds.
            unsafe { DestroyWindow(hwnd) }
                .map_err(|cause| DesktopError::new("destroy_main_window", cause))?;
            self.window = None;
            return Ok(None);
        }
        if self.events.as_ref().is_some_and(|events| events.in_callback()) {
            return Ok(None);
        }
        self.events = None;
        self.readiness = None;
        self.native_storage = None;
        self.request = None;
        self.retired = true;
        Ok(Some(DesktopStartupObservation::MainRetired(self.identity())))
    }
}

impl Drop for MainActor {
    /// Retain all unresolved native references and their credit before invoking a subscriber.
    fn drop(&mut self) {
        if !self.retired {
            std::mem::forget(self.request.take());
            std::mem::forget(self.window.take());
            std::mem::forget(self.presenter.take());
            std::mem::forget(self.controls.take());
            std::mem::forget(self.constructor.take());
            std::mem::forget(self.partial.take());
            std::mem::forget(self.view.take());
            std::mem::forget(self.receivers.take());
            std::mem::forget(self.events.take());
            std::mem::forget(self.native_storage.take());
            tracing::error!(
                operation = "main_actor_unresolved_drop",
                "Retaining UI-affine main lifetime until process termination"
            );
        }
    }
}
