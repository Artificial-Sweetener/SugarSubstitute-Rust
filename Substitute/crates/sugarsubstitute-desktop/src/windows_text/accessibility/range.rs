// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Every native range, clone, find result and selection operation retains the paint/retirement guard.
use super::*;
use std::mem::ManuallyDrop;

#[implement(abi::Range2, Agile = false)]
pub(super) struct Range {
    shared: Rc<Shared>,
    native: Rc<Lease>,
}
pub(super) fn wrap(
    shared: &Rc<Shared>,
    native: ITextRangeProvider,
) -> windows::core::Result<ITextRangeProvider> {
    let native = shared.lease(native.cast()?)?;
    let range: abi::Range2 = Range { shared: shared.clone(), native }.into();
    range.cast()
}
/// Intercept nullable range outputs at ABI level before any interface can reach a caller.
pub(super) unsafe fn output(
    shared: &Rc<Shared>,
    out: *mut *mut c_void,
    call: impl FnOnce() -> windows::core::Result<HRESULT>,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    // SAFETY: The ABI output is initialized before a fallible guard or native invocation.
    unsafe {
        out.write(std::ptr::null_mut());
    }
    let result: windows::core::Result<HRESULT> = (|| {
        shared.check()?;
        let code = call()?;
        // SAFETY: Native COM either left the initialized null or wrote one owned range reference.
        let pointer = unsafe { out.replace(std::ptr::null_mut()) };
        let native = if pointer.is_null() {
            None
        } else {
            // SAFETY: The non-null native output transferred an owned range reference.
            Some(unsafe { ITextRangeProvider::from_raw(pointer) })
        };
        code.ok()?;
        if let Some(native) = native {
            let wrapped = wrap(shared, native)?;
            // SAFETY: Only a guarded owned reference reaches the caller's output.
            unsafe {
                out.write(wrapped.into_raw());
            }
        }
        Ok(code)
    })();
    match result {
        Ok(code) => code,
        Err(cause) => cause.into(),
    }
}
impl Range {
    fn native(&self) -> windows::core::Result<ITextRangeProvider> {
        self.shared.check()?;
        self.native.get()
    }
    fn other(&self, pointer: *mut c_void) -> windows::core::Result<ITextRangeProvider> {
        self.shared.check()?;
        // SAFETY: COM supplies one borrowed nullable range argument for this call.
        let range =
            unsafe { ITextRangeProvider::from_raw_borrowed(&pointer) }.ok_or(E_INVALIDARG)?;
        let other = range.cast_object_ref::<Range>()?;
        if !Rc::ptr_eq(&self.shared, &other.shared) {
            return Err(E_INVALIDARG.into());
        }
        other.native()
    }
}
macro_rules! scalar {
    ($method:ident => $name:ident($($argument:ident:$kind:ty),*)->$result:ty) => {
        unsafe fn $method(&self,$($argument:$kind,)*out:*mut $result)->HRESULT {
            // SAFETY: Guarded native scalar operations receive the checked caller arguments.
            unsafe { arrays::output(out,|| {self.native()?.$name($($argument),*)}) }
        }
    };
}
macro_rules! action {
    ($method:ident => $name:ident($($argument:ident:$kind:ty),*))=>{
        unsafe fn $method(&self,$($argument:$kind),*)->HRESULT {
            let result=(||{
                let native=self.native()?;
                // SAFETY: Only the authoritative native range performs this guarded operation.
                unsafe { native.$name($($argument),*) }
            })();
            result.into()
        }
    };
}
impl abi::Range_Impl for Range_Impl {
    unsafe fn clone_range(&self, out: *mut *mut c_void) -> HRESULT {
        // SAFETY: The owned native clone is recursively wrapped.
        unsafe {
            output(&self.shared, out, || {
                let native = self.native()?;
                Ok((native.vtable().Clone)(native.as_raw(), out))
            })
        }
    }
    unsafe fn compare(&self, other: *mut c_void, out: *mut BOOL) -> HRESULT {
        // SAFETY: Only a recognized same-document range is unwrapped.
        unsafe { arrays::output(out, || self.native()?.Compare(&self.other(other)?)) }
    }
    unsafe fn compare_endpoints(
        &self,
        endpoint: TextPatternRangeEndpoint,
        other: *mut c_void,
        target: TextPatternRangeEndpoint,
        out: *mut i32,
    ) -> HRESULT {
        // SAFETY: Endpoint operations retain the native text engine's range semantics.
        unsafe {
            arrays::output(out, || {
                self.native()?.CompareEndpoints(endpoint, &self.other(other)?, target)
            })
        }
    }
    action!(expand_to_enclosing_unit => ExpandToEnclosingUnit(unit:TextUnit));
    unsafe fn find_attribute(
        &self,
        id: UIA_TEXTATTRIBUTE_ID,
        value: ManuallyDrop<VARIANT>,
        backward: BOOL,
        out: *mut *mut c_void,
    ) -> HRESULT {
        // SAFETY: VARIANT is borrowed by COM value; ManuallyDrop prevents freeing the caller's data.
        unsafe {
            output(&self.shared, out, || {
                let native = self.native()?;
                Ok((native.vtable().FindAttribute)(
                    native.as_raw(),
                    id,
                    std::ptr::read(&*value),
                    backward,
                    out,
                ))
            })
        }
    }
    unsafe fn find_text(
        &self,
        text: *mut c_void,
        backward: BOOL,
        ignore_case: BOOL,
        out: *mut *mut c_void,
    ) -> HRESULT {
        // SAFETY: The borrowed BSTR is forwarded without taking or freeing its ownership.
        unsafe {
            output(&self.shared, out, || {
                let native = self.native()?;
                Ok((native.vtable().FindText)(native.as_raw(), text, backward, ignore_case, out))
            })
        }
    }
    unsafe fn get_attribute_value(&self, id: UIA_TEXTATTRIBUTE_ID, out: *mut VARIANT) -> HRESULT {
        // SAFETY: Native attribute VARIANTs are checked for escaping interface payloads.
        unsafe { arrays::output(out, || arrays::property(self.native()?.GetAttributeValue(id)?)) }
    }
    scalar!(get_bounding_rectangles => GetBoundingRectangles()->*mut SAFEARRAY);
    unsafe fn get_enclosing_element(&self, out: *mut *mut c_void) -> HRESULT {
        // SAFETY: The actual native enclosing provider must match the sole bound text document.
        unsafe {
            arrays::output(out, || {
                let native = self.native()?.GetEnclosingElement()?;
                let child: IRawElementProviderSimple = self.shared.element(false)?;
                let object = child.cast_object_ref::<provider::TextElement>()?;
                let expected: IUnknown = object.element.native.get()?;
                if native.cast::<IUnknown>()?.as_raw() != expected.as_raw() {
                    return Err(E_INVALIDARG.into());
                }
                Ok(child.into_raw())
            })
        }
    }
    scalar!(get_text => GetText(maximum:i32)->BSTR);
    scalar!(r#move => Move(unit:TextUnit,count:i32)->i32);
    scalar!(move_endpoint_by_unit => MoveEndpointByUnit(endpoint:TextPatternRangeEndpoint,unit:TextUnit,count:i32)->i32);
    unsafe fn move_endpoint_by_range(
        &self,
        endpoint: TextPatternRangeEndpoint,
        other: *mut c_void,
        target: TextPatternRangeEndpoint,
    ) -> HRESULT {
        let result = (|| {
            // SAFETY: Native operations receive only checked same-document original ranges.
            unsafe { self.native()?.MoveEndpointByRange(endpoint, &self.other(other)?, target) }
        })();
        result.into()
    }
    action!(select => Select());
    action!(add_to_selection => AddToSelection());
    action!(remove_from_selection => RemoveFromSelection());
    unsafe fn scroll_into_view(&self, top: BOOL) -> HRESULT {
        let result = (|| {
            let native = self.native()?;
            // SAFETY: The native document performs scrolling for this exact retained range.
            unsafe { native.ScrollIntoView(top.as_bool()) }
        })();
        result.into()
    }
    unsafe fn get_children(&self, out: *mut *mut SAFEARRAY) -> HRESULT {
        // SAFETY: Read-only log text has no embedded objects. Reject unexpected native descendants.
        unsafe {
            arrays::output(out, || {
                let array = self.native()?.GetChildren()?;
                if array.is_null() {
                    return Ok(array);
                }
                let result = (|| {
                    use windows::Win32::System::Ole::{
                        SafeArrayGetDim, SafeArrayGetLBound, SafeArrayGetUBound,
                    };
                    if SafeArrayGetDim(array) != 1
                        || SafeArrayGetUBound(array, 1)? >= SafeArrayGetLBound(array, 1)?
                    {
                        return Err(E_INVALIDARG.into());
                    }
                    Ok(array)
                })();
                if result.is_err() {
                    arrays::destroy(array);
                }
                result
            })
        }
    }
}
impl abi::Range2_Impl for Range_Impl {
    unsafe fn show_context_menu(&self) -> HRESULT {
        let result = (|| {
            self.shared.check()?;
            let native: ITextRangeProvider2 = self.native.get()?;
            // SAFETY: The native text owner retains context menu and selection semantics.
            unsafe { native.ShowContextMenu() }
        })();
        result.into()
    }
}
