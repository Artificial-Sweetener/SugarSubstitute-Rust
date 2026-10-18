// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A single document's native UIA provider is guarded through every escaping COM interface.
//!
//! The custom child owns this STA boundary. Publication stops during semantic paint, and checked
//! disconnection revokes all retained native pattern/range references before the text engine retires.
use crate::DesktopError;
use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    rc::{Rc, Weak},
};
use sugarsubstitute_contracts::{
    execution::MemoryClaim, presentation::NATIVE_CONSOLE_ACCESSIBILITY_BYTES,
};
use windows::{
    Win32::{
        Foundation::{
            E_INVALIDARG, E_OUTOFMEMORY, E_POINTER, HMODULE, HWND, LPARAM, LRESULT, WPARAM,
        },
        System::{
            Com::SAFEARRAY, LibraryLoader::GetProcAddress, Threading::GetCurrentThreadId,
            Variant::VARIANT,
        },
        UI::{Accessibility::*, Controls::RichEdit::ITextServices2},
    },
    core::{BSTR, HRESULT, IUnknown, Interface, PCSTR, implement},
};
use windows_core::BOOL;
#[path = "accessibility/abi.rs"]
mod abi;
#[path = "accessibility/arrays.rs"]
mod arrays;
#[path = "accessibility/provider.rs"]
mod provider;
#[path = "accessibility/range.rs"]
mod range;
#[path = "accessibility/storage.rs"]
mod storage;
#[cfg(test)]
#[path = "accessibility/storage_tests.rs"]
mod storage_tests;
#[cfg(test)]
#[path = "accessibility/tests.rs"]
mod tests;
#[path = "accessibility/text.rs"]
mod text;

/// Native references can be revoked even while an external client retains a guarded wrapper.
struct Lease(RefCell<Option<IUnknown>>);
impl Lease {
    /// Borrow an owned native interface only until its checked synchronous operation returns.
    fn get<T: Interface>(&self) -> windows::core::Result<T> {
        let native = self.0.borrow().as_ref().cloned().ok_or_else(unavailable)?;
        native.cast()
    }
}

/// All externally retained COM wrappers share one STA, paint transaction and retirement state.
struct Shared {
    hwnd: HWND,
    thread: u32,
    painting: Rc<Cell<bool>>,
    closing: Cell<bool>,
    label: &'static str,
    root: Cell<*mut c_void>,
    child: Cell<*mut c_void>,
    leases: RefCell<Vec<Weak<Lease>>>,
    _claim: MemoryClaim,
}
impl Shared {
    /// Reject paint-time, retired and wrong-thread calls before accessing any native document.
    fn check(&self) -> windows::core::Result<()> {
        // SAFETY: GetCurrentThreadId observes the calling thread and has no borrowed native state.
        let thread = unsafe { GetCurrentThreadId() };
        if self.closing.get() || self.painting.get() || thread != self.thread {
            Err(unavailable())
        } else {
            Ok(())
        }
    }
    /// Bound independently retained native references and keep them collectively revocable.
    fn lease(&self, native: IUnknown) -> windows::core::Result<Rc<Lease>> {
        self.check()?;
        let mut leases = self.leases.borrow_mut();
        leases.retain(|old| old.strong_count() != 0);
        if leases.len() >= storage::MAX_LEASES {
            return Err(E_OUTOFMEMORY.into());
        }
        let lease = Rc::new(Lease(RefCell::new(Some(native))));
        leases.push(Rc::downgrade(&lease));
        Ok(lease)
    }
    /// Drop all native references after published UIA identity has disconnected successfully.
    fn revoke(&self) {
        let leases = std::mem::take(&mut *self.leases.borrow_mut());
        for lease in leases.into_iter().filter_map(|lease| lease.upgrade()) {
            lease.0.borrow_mut().take();
        }
        self.root.set(std::ptr::null_mut());
        self.child.set(std::ptr::null_mut());
    }
    /// Return an owned interface for one of the two exact retained fragment identities.
    fn element<T: Interface>(&self, root: bool) -> windows::core::Result<T> {
        self.check()?;
        let pointer = if root { self.root.get() } else { self.child.get() };
        // SAFETY: Accessibility retains both exact COM owners until checked disconnection;
        // closure rejects access before either pointer is cleared and owner references retire.
        unsafe { IUnknown::from_raw_borrowed(&pointer) }.ok_or_else(unavailable)?.cast()
    }
}
/// UIA's standard retired-element error also prevents observation of temporary paint tokens.
fn unavailable() -> windows::core::Error {
    HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32).into()
}

/// Keep the root, text child, native provider and windowless site alive through checked disconnect.
pub(super) struct Accessibility {
    shared: Rc<Shared>,
    root: Option<IRawElementProviderSimple>,
    child: Option<IRawElementProviderSimple>,
    native: Option<Rc<Lease>>,
    site: Option<IRawElementProviderWindowlessSite>,
    published: Cell<bool>,
}
impl Accessibility {
    /// Bind the sole native document. The caller retains the exact system module through close.
    pub(super) fn new(
        hwnd: HWND,
        services: &ITextServices2,
        module: HMODULE,
        painting: Rc<Cell<bool>>,
        label: &'static str,
        claim: MemoryClaim,
    ) -> Result<Self, DesktopError> {
        if claim.bytes() < NATIVE_CONSOLE_ACCESSIBILITY_BYTES
            || storage::AUTHORED_BYTES as u64 > claim.bytes()
        {
            return Err(DesktopError::message(
                "console_accessibility_claim",
                "admitted UI Automation storage is insufficient",
            ));
        }
        Self::create(hwnd, services, module, painting, label, claim)
            .map_err(|cause| DesktopError::new("console_accessibility_create", cause))
    }
    fn create(
        hwnd: HWND,
        services: &ITextServices2,
        module: HMODULE,
        painting: Rc<Cell<bool>>,
        label: &'static str,
        claim: MemoryClaim,
    ) -> windows::core::Result<Self> {
        let inventory = storage::inventory()?;
        // SAFETY: This is an IID data export from the retained system module, not a function call.
        let address = unsafe {
            GetProcAddress(module, PCSTR(c"IID_IRicheditWindowlessAccessibility".as_ptr().cast()))
        }
        .ok_or_else(windows::core::Error::from_thread)?;
        // SAFETY: The SDK defines this exact export as a readable GUID for the module lifetime.
        let iid = unsafe { *(address as *const windows::core::GUID) };
        let mut pointer = std::ptr::null_mut();
        // SAFETY: COM QueryInterface writes one owned reference for the exported IID.
        unsafe {
            (services.vtable().base__.base__.QueryInterface)(services.as_raw(), &iid, &mut pointer)
        }
        .ok()?;
        if pointer.is_null() {
            return Err(E_POINTER.into());
        }
        // SAFETY: Successful QI returned the exact interface represented by this SDK type.
        let access = unsafe { IRicheditWindowlessAccessibility::from_raw(pointer) };
        let shared = Rc::new(Shared {
            hwnd,
            painting,
            label,
            closing: Cell::new(false),
            // SAFETY: The calling UI thread owns the new windowless provider.
            thread: unsafe { GetCurrentThreadId() },
            root: Cell::new(std::ptr::null_mut()),
            child: Cell::new(std::ptr::null_mut()),
            leases: RefCell::new(inventory),
            _claim: claim,
        });
        let site: abi::Site = provider::WindowlessSite { shared: shared.clone() }.into();
        let site: IRawElementProviderWindowlessSite = site.cast()?;
        // SAFETY: The retained site remains alive through native provider retirement.
        let native = shared.lease(unsafe { access.CreateProvider(&site) }?.cast()?)?;
        let root: abi::Simple = provider::Container {
            element: provider::Element {
                shared: shared.clone(),
                native: native.clone(),
                container: true,
            },
        }
        .into();
        let child: abi::Simple = provider::TextElement {
            element: provider::Element {
                shared: shared.clone(),
                native: native.clone(),
                container: false,
            },
        }
        .into();
        let root: IRawElementProviderSimple = root.cast()?;
        let child: IRawElementProviderSimple = child.cast()?;
        shared.root.set(root.as_raw());
        shared.child.set(child.as_raw());
        Ok(Self {
            shared,
            root: Some(root),
            child: Some(child),
            native: Some(native),
            site: Some(site),
            published: Cell::new(false),
        })
    }
    /// Preserve WM_GETOBJECT's original parameters for UIA and Microsoft Active Accessibility.
    pub(super) fn message(&self, wparam: WPARAM, lparam: LPARAM) -> Result<LRESULT, DesktopError> {
        self.shared
            .check()
            .map_err(|cause| DesktopError::new("console_accessibility_query", cause))?;
        let root = self.root.as_ref().ok_or_else(|| {
            DesktopError::message("console_accessibility_query", "provider retired")
        })?;
        // SAFETY: The exact live HWND and retained root answer this synchronous WM_GETOBJECT.
        let result = unsafe { UiaReturnRawElementProvider(self.shared.hwnd, wparam, lparam, root) };
        self.published.set(self.published.get() || result.0 != 0);
        Ok(result)
    }
    /// Revoke native references only after UIA confirms its published root is disconnected.
    pub(super) fn close(&mut self) -> Result<(), DesktopError> {
        self.shared.closing.set(true);
        if self.published.get()
            && let Some(root) = self.root.as_ref()
        {
            // SAFETY: Retained root identity is precisely the object previously published to UIA.
            if let Err(cause) = unsafe { UiaDisconnectProvider(root) } {
                if cause.code() != unavailable().code() {
                    return Err(DesktopError::new("console_accessibility_disconnect", cause));
                }
                tracing::debug!(
                    operation = "console_accessibility_disconnect",
                    "UI Automation provider is already unavailable"
                );
            }
            self.published.set(false);
        }
        self.shared.revoke();
        self.native.take();
        self.child.take();
        self.root.take();
        self.site.take();
        Ok(())
    }
}
impl Drop for Accessibility {
    fn drop(&mut self) {
        if self.root.is_some() {
            self.shared.closing.set(true);
            std::mem::forget(self.root.take());
            std::mem::forget(self.child.take());
            std::mem::forget(self.native.take());
            std::mem::forget(self.site.take());
            std::mem::forget(self.shared.clone());
            tracing::error!(
                operation = "console_accessibility_unresolved_drop",
                "Retaining undisconnected provider and native document custody"
            );
        }
    }
}
