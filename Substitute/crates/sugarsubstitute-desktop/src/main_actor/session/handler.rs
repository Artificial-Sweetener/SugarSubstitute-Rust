// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Register one top-document WebView2 receiver without entering an application callback.
use super::{Channel, queue};
use crate::DesktopError;
use webview2_com::WebMessageReceivedEventHandler;
use windows::core::PWSTR;
impl Channel {
    pub(super) fn install_handler(&mut self) -> Result<(), DesktopError> {
        // SAFETY: This actual creating-thread core returns its own settings; the session bridge
        // explicitly enables the native web-message channel before exposing its descriptor.
        let settings = unsafe { self.core.Settings() }
            .map_err(|cause| DesktopError::new("session_message_settings", cause))?;
        // SAFETY: UI-affine settings belong to the retained current native view.
        unsafe { settings.SetIsWebMessageEnabled(true) }
            .map_err(|cause| DesktopError::new("enable_session_messages", cause))?;
        let queue = self.queue.clone();
        let events = self.events.clone();
        let handler = WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
            let _callback = events.callback();
            let Some(args) = args else {
                events.fail(DesktopError::message("session_message", "missing native event"));
                return Ok(());
            };
            let mut source = PWSTR::null();
            let mut payload = PWSTR::null();
            // SAFETY: Live native event args write owned CoTaskMem strings; both are adopted before inspection.
            let source_result = unsafe { args.Source(&mut source) };
            let source = crate::windows_main_receivers::NativeString(source);
            // SAFETY: As above; no authored queue borrow spans either native getter.
            let payload_result = unsafe { args.WebMessageAsJson(&mut payload) };
            let payload = crate::windows_main_receivers::NativeString(payload);
            match source_result.and(payload_result) {
                Err(cause) => events.fail(DesktopError::new("get_session_message", cause)),
                Ok(()) if source.0.is_null() || payload.0.is_null() => events
                    .fail(DesktopError::message("get_session_message", "null native event text")),
                Ok(()) => queue.record(
                    |output| {
                        queue::copy(output, queue::SOURCE_BYTES, |index| {
                            // SAFETY: Both getters completed; owned NUL-terminated native storage remains live.
                            unsafe { source.0.as_ptr().add(index).read() }
                        })
                    },
                    |output| {
                        queue::copy(output, queue::FRAME_BYTES, |index| {
                            // SAFETY: The same owned native result remains live until copying completes.
                            unsafe { payload.0.as_ptr().add(index).read() }
                        })
                    },
                ),
            }
            Ok(())
        }));
        let mut token = 0;
        // SAFETY: The exact UI-affine core owns this registration; adopt its token before interpreting HRESULT.
        let result = unsafe { self.core.add_WebMessageReceived(&handler, &mut token) };
        if result.is_ok() || token != 0 {
            self.handler = Some(token);
        } else {
            self.uncertain = true;
        }
        result.map_err(|cause| DesktopError::new("install_session_receiver", cause))
    }
}
