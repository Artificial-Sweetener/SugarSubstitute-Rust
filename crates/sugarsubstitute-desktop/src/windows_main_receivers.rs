// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain native CDP receiver registrations and actual asynchronous callback obligations.
//!
//! Every method/interface/callback is UI-affine. The outer STA and parent outlive this
//! owner. Native getters return before queue borrowing; no callback drives construction,
//! navigation, cleanup or an event loop. Checked removal preserves failed native slots.

mod completion;
mod downloads;
mod navigation;
use crate::{
    DesktopError,
    main_events::{Command, Events, Kind},
};
#[cfg(feature = "k1-native-qualification")]
pub(crate) use downloads::qualification::DownloadWitness;
use std::rc::Rc;
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
use webview2_com::{DevToolsProtocolEventReceivedEventHandler, Microsoft::Web::WebView2::Win32::*};
use windows::{
    Win32::{Foundation::E_POINTER, System::Com::CoTaskMemFree},
    core::{HSTRING, Interface, PWSTR},
};

const SOURCES: [(&str, Kind); 7] = [
    ("Page.frameNavigated", Kind::Frame),
    ("Page.frameDetached", Kind::FrameDetached),
    ("Runtime.executionContextCreated", Kind::Context),
    ("Runtime.executionContextDestroyed", Kind::ContextDestroyed),
    ("Runtime.executionContextsCleared", Kind::ContextsCleared),
    ("Runtime.bindingCalled", Kind::PanelBinding),
    ("Runtime.consoleAPICalled", Kind::Console),
];

struct Subscription {
    receiver: Option<ICoreWebView2DevToolsProtocolEventReceiver>,
    token: Option<i64>,
    uncertain: bool,
}

#[cfg(test)]
#[path = "windows_main_receivers/source_tests.rs"]
mod source_tests;

/// Own a returned CoTaskMem string immediately, including failed native getters.
pub(super) struct NativeString(pub(super) PWSTR);
impl Drop for NativeString {
    fn drop(&mut self) {
        // SAFETY: The getter transfers CoTaskMem ownership, including a partial failure output.
        unsafe {
            CoTaskMemFree(Some(self.0.as_ptr().cast()));
        }
    }
}

/// Retain partial native registrations even when installation reports failure.
pub(crate) struct Receivers {
    subscriptions: Vec<Subscription>,
    navigation: navigation::Navigation,
    downloads: Rc<downloads::Owner>,
    operations: Rc<completion::Operations>,
    events: Rc<Events>,
    core: ICoreWebView2,
    _claim: MemoryClaim,
}
impl Receivers {
    /// Claim all known registration/command metadata before creating receivers.
    pub(crate) fn new(
        core: ICoreWebView2,
        events: Rc<Events>,
        lease: &MemoryLease,
        origin: sugarsubstitute_contracts::configuration::LoopbackOrigin,
    ) -> Result<Self, DesktopError> {
        let claim =
            lease.claim(65536).map_err(|cause| DesktopError::new("claim_main_receivers", cause))?;
        let mut owner = Self {
            subscriptions: Vec::new(),
            navigation: navigation::Navigation::new(origin),
            downloads: downloads::Owner::new(origin)?,
            operations: completion::Operations::new(events.clone())?,
            events,
            core,
            _claim: claim,
        };
        owner
            .subscriptions
            .try_reserve_exact(SOURCES.len())
            .map_err(|cause| DesktopError::new("allocate_main_receivers", cause))?;
        if owner.subscriptions.capacity() != SOURCES.len() {
            return Err(DesktopError::message(
                "allocate_main_receivers",
                "receiver capacity exceeded",
            ));
        }
        Ok(owner)
    }

    /// Install one receiver per normal loop drain. Failed outputs remain in this owner.
    pub(crate) fn install_next(&mut self) -> Result<bool, DesktopError> {
        self.require_outer_call()?;
        if self.subscriptions.len() == SOURCES.len() {
            return self.navigation.install(&self.core, &self.events, &self.downloads);
        }
        let (name, kind) = SOURCES[self.subscriptions.len()];
        let name = HSTRING::from(name);
        let mut raw = std::ptr::null_mut();
        // SAFETY: Valid bounded name and writable COM output; capture actual output before HRESULT.
        let status = unsafe {
            (self.core.vtable().GetDevToolsProtocolEventReceiver)(
                self.core.as_raw(),
                windows::core::PCWSTR(name.as_ptr()),
                &mut raw,
            )
        };
        let receiver = if raw.is_null() {
            None
        } else {
            // SAFETY: Native getter transfers this exact interface reference, including partial failure.
            Some(unsafe { ICoreWebView2DevToolsProtocolEventReceiver::from_raw(raw) })
        };
        self.subscriptions.push(Subscription { receiver, token: None, uncertain: false });
        status.ok().map_err(|cause| DesktopError::new("get_main_receiver", cause))?;
        let slot = self.subscriptions.last_mut().ok_or_else(|| {
            DesktopError::message("install_main_receiver", "missing retained receiver")
        })?;
        let receiver = slot.receiver.as_ref().ok_or_else(|| {
            DesktopError::new("get_main_receiver", windows::core::Error::from(E_POINTER))
        })?;
        let events = self.events.clone();
        let handler =
            DevToolsProtocolEventReceivedEventHandler::create(Box::new(move |_, args| {
                let _callback = events.callback();
                let mut output = PWSTR::null();
                // SAFETY: Valid native event interface/writable output, with no queue borrow held.
                let status = match args {
                    // SAFETY: The retained event interface writes this owned scalar output.
                    Some(args) => unsafe { args.ParameterObjectAsJson(&mut output) },
                    None => Err(windows::core::Error::from(E_POINTER)),
                };
                let returned = NativeString(output);
                if let Err(cause) = status {
                    events.fail(DesktopError::new("get_main_event_payload", cause));
                } else if returned.0.is_null() {
                    events.fail(DesktopError::new(
                        "get_main_event_payload",
                        windows::core::Error::from(E_POINTER),
                    ));
                } else {
                    events.record(kind, |target| {
                        crate::main_events::copy_utf16(target, |index| {
                            // SAFETY: Successful native getter supplies valid NUL-terminated storage.
                            unsafe { returned.0.as_ptr().add(index).read() }
                        })
                    });
                }
                Ok(())
            }));
        let mut token = 0;
        // SAFETY: Retained source owns callback reference; partial token is adopted before status.
        let installed = unsafe { receiver.add_DevToolsProtocolEventReceived(&handler, &mut token) };
        if installed.is_ok() || token != 0 {
            slot.token = Some(token);
        } else {
            slot.uncertain = true;
        }
        installed.map_err(|cause| DesktopError::new("install_main_receiver", cause))?;
        Ok(false)
    }

    /// Attach one bounded observer to production-admitted downloads for a fixed qualification.
    #[cfg(feature = "k1-native-qualification")]
    pub(crate) fn download_witness(&self) -> Result<Rc<DownloadWitness>, DesktopError> {
        self.require_outer_call()?;
        self.downloads.witness()
    }

    /// Keep browser command capacity available for panel and lifecycle traffic.
    pub(crate) fn admits_surface_delivery(&self) -> Result<bool, DesktopError> {
        self.require_outer_call()?;
        Ok(self.operations.admits_surface_delivery())
    }

    /// Submit at most four retained same-session CDP operations; callbacks alone retire them.
    pub(crate) fn call(
        &self,
        kind: Command,
        method: &str,
        parameters: &str,
    ) -> Result<(), DesktopError> {
        self.require_outer_call()?;
        completion::call(&self.core, &self.operations, kind, method, parameters)
    }

    /// Open only the exact current trusted frontend navigation after both authorization fences.
    pub(super) fn reveal_navigation(&self) {
        self.navigation.revealed();
    }
    pub(crate) fn begin_navigation(&self) -> Result<(), DesktopError> {
        self.navigation.begin()
    }

    /// Observe the actual current top native content for renderer qualification only.
    #[cfg(feature = "k1-native-qualification")]
    pub(crate) fn preview_navigation(&self) -> Result<u64, DesktopError> {
        self.navigation.preview_navigation()
    }

    /// Close evidence admission, then remove one actual receiver per normal loop drain.
    /// A pending callback, failed removal or uncertain registration retains all owners.
    pub(crate) fn retire_step(&mut self) -> Result<bool, DesktopError> {
        self.require_outer_call()?;
        self.events.close_admission();
        if !self.downloads.retire_step()? {
            return Ok(false);
        }
        if self.operations.pending() != 0 {
            return Ok(false);
        }
        if !self.navigation.retire_step(&self.core)? {
            return Ok(false);
        }
        if let Some(slot) = self.subscriptions.last_mut() {
            if slot.uncertain {
                return Ok(false);
            }
            if let Some(token) = slot.token {
                if let Some(receiver) = &slot.receiver {
                    // SAFETY: This exact token belongs to the retained receiver on its UI thread.
                    unsafe { receiver.remove_DevToolsProtocolEventReceived(token) }
                        .map_err(|cause| DesktopError::new("remove_main_receiver", cause))?;
                }
                slot.token = None;
            }
            self.subscriptions.pop();
            return Ok(false);
        }
        Ok(true)
    }

    fn require_outer_call(&self) -> Result<(), DesktopError> {
        if self.events.in_callback() {
            Err(DesktopError::message(
                "main_receiver_effect",
                "callback cannot drive native effects",
            ))
        } else {
            Ok(())
        }
    }
}
