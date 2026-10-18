// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Checked ownership of UIA arrays; no unguarded native interface may escape in an array.
use super::*;
use windows::Win32::System::{
    Ole::*,
    Variant::{VT_I4, VT_UNKNOWN},
};

pub(super) fn runtime_id(child: bool) -> windows::core::Result<*mut SAFEARRAY> {
    let values = [UiaAppendRuntimeId as i32, 4, i32::from(child) + 1];
    // SAFETY: This allocates a new owned, one-dimensional integer array.
    let array = unsafe { SafeArrayCreateVector(VT_I4, 0, values.len() as u32) };
    if array.is_null() {
        return Err(E_OUTOFMEMORY.into());
    }
    for (index, value) in values.iter().enumerate() {
        // SAFETY: Index is within the new integer array and value points to one initialized i32.
        let inserted =
            unsafe { SafeArrayPutElement(array, &(index as i32), (value as *const i32).cast()) };
        if let Err(cause) = inserted {
            destroy(array);
            return Err(cause);
        }
    }
    Ok(array)
}

pub(super) fn ranges(
    shared: &Rc<Shared>,
    array: *mut SAFEARRAY,
) -> windows::core::Result<*mut SAFEARRAY> {
    if array.is_null() {
        return Ok(array);
    }
    let result = (|| {
        // SAFETY: The sole native provider transferred this owned array to this boundary.
        let dimensions = unsafe { SafeArrayGetDim(array) };
        // SAFETY: The same native-owned array remains live throughout this scalar type query.
        let kind = unsafe { SafeArrayGetVartype(array) }?;
        if dimensions != 1 || kind != VT_UNKNOWN {
            return Err(E_INVALIDARG.into());
        }
        // SAFETY: The checked one-dimensional array has a valid first dimension.
        let low = unsafe { SafeArrayGetLBound(array, 1) }?;
        // SAFETY: The checked one-dimensional array has a valid first dimension.
        let high = unsafe { SafeArrayGetUBound(array, 1) }?;
        if i64::from(high) - i64::from(low) + 1 > 4096 {
            return Err(E_OUTOFMEMORY.into());
        }
        for index in low..=high {
            let mut pointer: *mut c_void = std::ptr::null_mut();
            // SAFETY: Each checked in-bounds IUnknown element returns an AddRef'd reference.
            unsafe { SafeArrayGetElement(array, &index, (&raw mut pointer).cast()) }?;
            if pointer.is_null() {
                return Err(E_POINTER.into());
            }
            // SAFETY: The successful array read transferred one owned IUnknown reference.
            let native = unsafe { IUnknown::from_raw(pointer) };
            let wrapped = range::wrap(shared, native.cast()?)?;
            // SAFETY: SafeArrayPutElement AddRefs the replacement before wrapped retires.
            unsafe { SafeArrayPutElement(array, &index, wrapped.as_raw()) }?;
        }
        Ok(array)
    })();
    if result.is_err() {
        destroy(array);
    }
    result
}

pub(super) fn destroy(array: *mut SAFEARRAY) {
    // SAFETY: Only newly returned arrays owned by this boundary reach this cleanup.
    if let Err(cause) = unsafe { SafeArrayDestroy(array) } {
        tracing::error!(operation="console_accessibility_array_retirement", error=%cause);
    }
}

/// Initialize each COM output even when the operation rejects a reentrant call.
pub(super) unsafe fn output<T: Default>(
    out: *mut T,
    body: impl FnOnce() -> windows::core::Result<T>,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    // SAFETY: The caller's non-null ABI output points to writable storage for T.
    unsafe {
        out.write(T::default());
    }
    match body() {
        Ok(value) => {
            // SAFETY: The same ABI output receives exactly one initialized owned result.
            unsafe {
                out.write(value);
            }
            HRESULT(0)
        }
        Err(cause) => cause.into(),
    }
}

pub(super) fn interface<T: Interface>(value: T) -> *mut c_void {
    value.into_raw()
}

/// Only UIA's immutable reserved sentinel objects may cross a VARIANT's interface boundary.
pub(super) fn property(value: VARIANT) -> windows::core::Result<VARIANT> {
    // SAFETY: The initialized VARIANT tag identifies its active payload without reading that payload.
    let tag = unsafe { value.Anonymous.Anonymous.vt }.0;
    if tag == 13 {
        let native = IUnknown::try_from(&value)?;
        // SAFETY: UIA's reserved immutable objects contain no document or native provider handles.
        let mixed = unsafe { UiaGetReservedMixedAttributeValue() }?;
        // SAFETY: This second sentinel similarly contains no document-specific state.
        let unsupported = unsafe { UiaGetReservedNotSupportedValue() }?;
        if native.as_raw() != mixed.as_raw() && native.as_raw() != unsupported.as_raw() {
            return Err(HRESULT(UIA_E_NOTSUPPORTED as i32).into());
        }
    } else if !matches!(tag & !0x2000, 0..=8 | 10 | 11 | 14 | 16..=23) {
        return Err(HRESULT(UIA_E_NOTSUPPORTED as i32).into());
    }
    Ok(value)
}
