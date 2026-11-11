// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake native COM boundaries while the real media registry owns admission and teardown.
use super::*;
use std::cell::Cell;
use windows::{
    Win32::{
        Foundation::{E_FAIL, E_NOTIMPL, HWND},
        System::Com::IStream,
    },
    core::{PCWSTR, PWSTR, Ref, implement},
};
#[derive(Default)]
pub(super) struct Witness {
    pub(super) assigned: Cell<u32>,
    pub(super) completed: Cell<u32>,
    pub(super) fail_complete: Cell<bool>,
    pub(super) status: Cell<i32>,
    pub(super) headers: RefCell<String>,
}
fn unsupported<T>() -> windows::core::Result<T> {
    Err(windows::core::Error::from(E_NOTIMPL))
}
#[implement(ICoreWebView2WebResourceRequestedEventArgs)]
struct Arguments {
    witness: Rc<Witness>,
    response: RefCell<Option<ICoreWebView2WebResourceResponse>>,
}
impl ICoreWebView2WebResourceRequestedEventArgs_Impl for Arguments_Impl {
    fn Request(&self) -> windows::core::Result<ICoreWebView2WebResourceRequest> {
        unsupported()
    }
    fn Response(&self) -> windows::core::Result<ICoreWebView2WebResourceResponse> {
        self.response.borrow().clone().ok_or_else(|| windows::core::Error::from(E_NOTIMPL))
    }
    fn SetResponse(
        &self,
        response: Ref<ICoreWebView2WebResourceResponse>,
    ) -> windows::core::Result<()> {
        self.witness.assigned.set(self.witness.assigned.get() + 1);
        *self.response.borrow_mut() = response.cloned();
        Ok(())
    }
    fn GetDeferral(&self) -> windows::core::Result<ICoreWebView2Deferral> {
        Ok(Deferral { witness: self.witness.clone() }.into())
    }
    fn ResourceContext(
        &self,
        _: *mut COREWEBVIEW2_WEB_RESOURCE_CONTEXT,
    ) -> windows::core::Result<()> {
        unsupported()
    }
}
#[implement(ICoreWebView2Deferral)]
struct Deferral {
    witness: Rc<Witness>,
}
impl ICoreWebView2Deferral_Impl for Deferral_Impl {
    fn Complete(&self) -> windows::core::Result<()> {
        self.witness.completed.set(self.witness.completed.get() + 1);
        if self.witness.fail_complete.replace(false) {
            Err(windows::core::Error::from(E_FAIL))
        } else {
            Ok(())
        }
    }
}
#[implement(ICoreWebView2Environment)]
struct Environment {
    witness: Rc<Witness>,
}
impl ICoreWebView2Environment_Impl for Environment_Impl {
    fn CreateCoreWebView2Controller(
        &self,
        _: HWND,
        _: Ref<ICoreWebView2CreateCoreWebView2ControllerCompletedHandler>,
    ) -> windows::core::Result<()> {
        unsupported()
    }
    fn CreateWebResourceResponse(
        &self,
        content: Ref<IStream>,
        status: i32,
        _: &PCWSTR,
        headers: &PCWSTR,
    ) -> windows::core::Result<ICoreWebView2WebResourceResponse> {
        self.witness.status.set(status);
        // SAFETY: The production response owner passes a live, bounded HSTRING for this synchronous call.
        *self.witness.headers.borrow_mut() = unsafe { headers.to_string() }?;
        Ok(Response { content: content.cloned() }.into())
    }
    fn BrowserVersionString(&self, _: *mut PWSTR) -> windows::core::Result<()> {
        unsupported()
    }
    fn add_NewBrowserVersionAvailable(
        &self,
        _: Ref<ICoreWebView2NewBrowserVersionAvailableEventHandler>,
        _: *mut i64,
    ) -> windows::core::Result<()> {
        unsupported()
    }
    fn remove_NewBrowserVersionAvailable(&self, _: i64) -> windows::core::Result<()> {
        unsupported()
    }
}
#[implement(ICoreWebView2WebResourceResponse)]
struct Response {
    content: Option<IStream>,
}
impl ICoreWebView2WebResourceResponse_Impl for Response_Impl {
    fn Content(&self) -> windows::core::Result<IStream> {
        self.content.clone().ok_or_else(|| windows::core::Error::from(E_NOTIMPL))
    }
    fn SetContent(&self, _: Ref<IStream>) -> windows::core::Result<()> {
        unsupported()
    }
    fn Headers(&self) -> windows::core::Result<ICoreWebView2HttpResponseHeaders> {
        unsupported()
    }
    fn StatusCode(&self, _: *mut i32) -> windows::core::Result<()> {
        unsupported()
    }
    fn SetStatusCode(&self, _: i32) -> windows::core::Result<()> {
        unsupported()
    }
    fn ReasonPhrase(&self, _: *mut PWSTR) -> windows::core::Result<()> {
        unsupported()
    }
    fn SetReasonPhrase(&self, _: &PCWSTR) -> windows::core::Result<()> {
        unsupported()
    }
}
pub(super) fn boundaries(
    witness: Rc<Witness>,
) -> (ICoreWebView2WebResourceRequestedEventArgs, ICoreWebView2Environment) {
    (
        Arguments { witness: witness.clone(), response: RefCell::new(None) }.into(),
        Environment { witness }.into(),
    )
}
