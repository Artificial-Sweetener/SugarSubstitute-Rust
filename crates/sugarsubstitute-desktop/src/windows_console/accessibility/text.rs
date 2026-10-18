// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Guard native Text, Text2, TextEdit and read-only Value patterns, including retained ranges.
use super::*;
use windows::core::PCWSTR;

#[implement(abi::Text2, abi::TextEdit, Agile = false)]
pub(super) struct Text {
    pub(super) shared: Rc<Shared>,
    pub(super) native: Rc<Lease>,
}
#[implement(IValueProvider, Agile = false)]
pub(super) struct Value {
    pub(super) shared: Rc<Shared>,
    pub(super) native: Rc<Lease>,
}

impl Text {
    fn native<T: Interface>(&self) -> windows::core::Result<T> {
        self.shared.check()?;
        self.native.get()
    }
    fn child(&self, pointer: *mut c_void) -> windows::core::Result<IRawElementProviderSimple> {
        self.shared.check()?;
        // SAFETY: COM supplies a borrowed nullable provider for this call; no ownership is taken.
        let child = unsafe { IRawElementProviderSimple::from_raw_borrowed(&pointer) }
            .ok_or(E_INVALIDARG)?;
        let object = child.cast_object_ref::<provider::TextElement>()?;
        if !Rc::ptr_eq(&object.element.shared, &self.shared) {
            return Err(E_INVALIDARG.into());
        }
        object.element.native.get()
    }
}
impl abi::Text_Impl for Text_Impl {
    unsafe fn get_selection(&self, out: *mut *mut SAFEARRAY) -> HRESULT {
        // SAFETY: Native owned arrays are recursively wrapped before reaching the initialized output.
        unsafe {
            arrays::output(out, || {
                let native: ITextProvider = self.native()?;
                arrays::ranges(&self.shared, native.GetSelection()?)
            })
        }
    }
    unsafe fn get_visible_ranges(&self, out: *mut *mut SAFEARRAY) -> HRESULT {
        // SAFETY: No native interface is published before array rewriting completes.
        unsafe {
            arrays::output(out, || {
                let native: ITextProvider = self.native()?;
                arrays::ranges(&self.shared, native.GetVisibleRanges()?)
            })
        }
    }
    unsafe fn range_from_child(&self, child: *mut c_void, out: *mut *mut c_void) -> HRESULT {
        // SAFETY: Only a recognized same-document child is unwrapped; the result is guarded.
        unsafe {
            range::output(&self.shared, out, || {
                let native: ITextProvider = self.native()?;
                let child = self.child(child)?;
                Ok((native.vtable().RangeFromChild)(native.as_raw(), child.as_raw(), out))
            })
        }
    }
    unsafe fn range_from_point(&self, point: UiaPoint, out: *mut *mut c_void) -> HRESULT {
        // SAFETY: The native provider alone maps screen coordinates to document positions.
        unsafe {
            range::output(&self.shared, out, || {
                let native: ITextProvider = self.native()?;
                Ok((native.vtable().RangeFromPoint)(native.as_raw(), point, out))
            })
        }
    }
    unsafe fn document_range(&self, out: *mut *mut c_void) -> HRESULT {
        // SAFETY: The document range is wrapped before its native reference escapes.
        unsafe {
            range::output(&self.shared, out, || {
                let native: ITextProvider = self.native()?;
                Ok((native.vtable().DocumentRange)(native.as_raw(), out))
            })
        }
    }
    unsafe fn supported_text_selection(&self, out: *mut SupportedTextSelection) -> HRESULT {
        // SAFETY: The native selection capability is read only after the paint guard.
        unsafe {
            arrays::output(out, || {
                let native: ITextProvider = self.native()?;
                native.SupportedTextSelection()
            })
        }
    }
}
impl abi::Text2_Impl for Text_Impl {
    unsafe fn range_from_annotation(&self, child: *mut c_void, out: *mut *mut c_void) -> HRESULT {
        // SAFETY: Foreign providers cannot cross document identity; returned ranges are guarded.
        unsafe {
            range::output(&self.shared, out, || {
                let native: ITextProvider2 = self.native()?;
                let child = self.child(child)?;
                Ok((native.vtable().RangeFromAnnotation)(native.as_raw(), child.as_raw(), out))
            })
        }
    }
    unsafe fn get_caret_range(&self, active: *mut BOOL, out: *mut *mut c_void) -> HRESULT {
        if active.is_null() {
            return E_POINTER;
        }
        // SAFETY: Required ABI output is initialized even if guard/native access fails.
        unsafe {
            active.write(BOOL(0));
            range::output(&self.shared, out, || {
                let native: ITextProvider2 = self.native()?;
                Ok((native.vtable().GetCaretRange)(native.as_raw(), active, out))
            })
        }
    }
}
impl abi::TextEdit_Impl for Text_Impl {
    unsafe fn get_active_composition(&self, out: *mut *mut c_void) -> HRESULT {
        // SAFETY: A nullable native composition range is wrapped before publication.
        unsafe {
            range::output(&self.shared, out, || {
                let native: ITextEditProvider = self.native()?;
                Ok((native.vtable().GetActiveComposition)(native.as_raw(), out))
            })
        }
    }
    unsafe fn get_conversion_target(&self, out: *mut *mut c_void) -> HRESULT {
        // SAFETY: A nullable native conversion range is wrapped before publication.
        unsafe {
            range::output(&self.shared, out, || {
                let native: ITextEditProvider = self.native()?;
                Ok((native.vtable().GetConversionTarget)(native.as_raw(), out))
            })
        }
    }
}
impl IValueProvider_Impl for Value_Impl {
    fn SetValue(&self, value: &PCWSTR) -> windows::core::Result<()> {
        self.shared.check()?;
        let native: IValueProvider = self.native.get()?;
        // SAFETY: The actual read-only RichEdit provider authoritatively rejects modification.
        unsafe { native.SetValue(*value) }
    }
    fn Value(&self) -> windows::core::Result<BSTR> {
        self.shared.check()?;
        let native: IValueProvider = self.native.get()?;
        // SAFETY: The native document returns an owned Unicode string outside paint.
        unsafe { native.Value() }
    }
    fn IsReadOnly(&self) -> windows::core::Result<BOOL> {
        self.shared.check()?;
        let native: IValueProvider = self.native.get()?;
        // SAFETY: The actual document reports its native read-only state.
        unsafe { native.IsReadOnly() }
    }
}
