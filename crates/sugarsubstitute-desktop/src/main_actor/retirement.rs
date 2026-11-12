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
        self.placement.close();
        if let Some(session) = &mut self.session {
            session.close();
        }
        self.panel_transport.close();
        self.bulk.close();
        self.canvas_replies.clear();
        self.panel_messages.clear();
        self.recovery.close();
        #[cfg(feature = "native-composition")]
        if let Some(composition) = &self.composition {
            composition.suspend_input();
        }
        let retry_failure = self.present_recovery_control().err();
        #[cfg(feature = "k1-native-qualification")]
        if let Some(preview) = &mut self.preview {
            preview.close();
        }
        if let Some(readiness) = &mut self.readiness {
            readiness.invalidate(ReadinessFault::Cancelled);
        }
        if let Some(events) = &self.events {
            events.close_admission();
        }
        if let Some(constructor) = &mut self.constructor {
            constructor.cancel();
        }
        match retry_failure {
            Some(cause) => Err(cause),
            None => Ok(()),
        }
    }

    /// A true result requires every native receiver, creation callback, controller and HWND retired.
    pub(super) fn retire_step(
        &mut self,
    ) -> Result<Option<DesktopStartupObservation>, DesktopError> {
        if let Some(session) = &mut self.session {
            if let Some(observation) = session.step()? {
                return Ok(Some(observation));
            }
            if session.retire_step()? {
                self.session = None;
            }
            return Ok(None);
        }
        if let Some(bridge) = &mut self.panel_bridge {
            if bridge.retire_step()? {
                self.panel_bridge = None;
            }
            return Ok(None);
        }
        #[cfg(feature = "k1-native-qualification")]
        if let Some(preview) = &mut self.preview
            && !preview.retirement_ready()?
        {
            return Ok(None);
        }
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
        #[cfg(feature = "native-composition")]
        if let Some(composition) = &mut self.composition {
            if composition.retire_step()? {
                self.composition = None;
            }
            return Ok(None);
        }
        if let Some(focus) = &self.windowed_focus {
            focus.retire()?;
            self.windowed_focus = None;
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
            #[cfg(feature = "k1-native-qualification")]
            {
                self.preview = None;
            }
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
            if self.owned_window_open {
                return Ok(None);
            }
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
        #[cfg(feature = "native-composition")]
        {
            self.reused_environment = None;
        }
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
            #[cfg(feature = "k1-native-qualification")]
            std::mem::forget(self.preview.take());
            std::mem::forget(self.request.take());
            std::mem::forget(self.window.take());
            std::mem::forget(self.presenter.take());
            std::mem::forget(self.controls.take());
            std::mem::forget(self.constructor.take());
            std::mem::forget(self.partial.take());
            std::mem::forget(self.windowed_focus.take());
            std::mem::forget(self.view.take());
            #[cfg(feature = "native-composition")]
            std::mem::forget(self.composition.take());
            #[cfg(feature = "native-composition")]
            std::mem::forget(self.reused_environment.take());
            std::mem::forget(self.panel_bridge.take());
            std::mem::forget(self.session.take());
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
