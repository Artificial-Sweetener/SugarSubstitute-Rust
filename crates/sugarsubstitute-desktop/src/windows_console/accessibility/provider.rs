// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The child HWND is one fragment container with one authoritative native text document.
use super::*;
use windows::Win32::{
    Foundation::RECT,
    UI::{
        Input::KeyboardAndMouse::{GetFocus, IsWindowEnabled, SetFocus},
        WindowsAndMessaging::GetWindowRect,
    },
};

pub(super) struct Element {
    pub(super) shared: Rc<Shared>,
    pub(super) native: Rc<Lease>,
    pub(super) container: bool,
}
#[implement(abi::Simple, abi::Fragment, abi::Root, Agile = false)]
pub(super) struct Container {
    pub(super) element: Element,
}
#[implement(abi::Simple, abi::Fragment, Agile = false)]
pub(super) struct TextElement {
    pub(super) element: Element,
}
#[implement(abi::Site, Agile = false)]
pub(super) struct WindowlessSite {
    pub(super) shared: Rc<Shared>,
}

impl Element {
    fn bounds(&self) -> windows::core::Result<UiaRect> {
        self.shared.check()?;
        let mut rect = RECT::default();
        // SAFETY: The checked live UI-thread child owns its physical screen rectangle.
        unsafe { GetWindowRect(self.shared.hwnd, &mut rect) }?;
        Ok(UiaRect {
            left: f64::from(rect.left),
            top: f64::from(rect.top),
            width: f64::from(rect.right - rect.left),
            height: f64::from(rect.bottom - rect.top),
        })
    }
    fn pattern(&self, id: UIA_PATTERN_ID) -> windows::core::Result<*mut c_void> {
        self.shared.check()?;
        if self.container {
            return Ok(std::ptr::null_mut());
        }
        let provider: IRawElementProviderSimple = self.native.get()?;
        let mut pointer = std::ptr::null_mut();
        // SAFETY: The exact retained native provider returns an owned nullable pattern reference.
        unsafe { (provider.vtable().GetPatternProvider)(provider.as_raw(), id, &mut pointer) }
            .ok()?;
        if pointer.is_null() {
            return Ok(pointer);
        }
        // SAFETY: A successful non-null native result transfers one IUnknown reference.
        let native = unsafe { IUnknown::from_raw(pointer) };
        let lease = self.shared.lease(native)?;
        let wrapped: IUnknown =
            if id == UIA_TextPatternId || id == UIA_TextPattern2Id || id == UIA_TextEditPatternId {
                let object: abi::Text2 =
                    text::Text { shared: self.shared.clone(), native: lease }.into();
                object.cast()?
            } else if id == UIA_ValuePatternId {
                let object: IValueProvider =
                    text::Value { shared: self.shared.clone(), native: lease }.into();
                object.cast()?
            } else {
                return Err(HRESULT(UIA_E_NOTSUPPORTED as i32).into());
            };
        Ok(wrapped.into_raw())
    }
    fn property(&self, id: UIA_PROPERTY_ID) -> windows::core::Result<VARIANT> {
        self.shared.check()?;
        if id == UIA_NamePropertyId {
            return Ok(VARIANT::from(self.shared.label));
        }
        if id == UIA_ControlTypePropertyId && self.container {
            return Ok(VARIANT::from(UIA_GroupControlTypeId.0));
        }
        if id == UIA_HasKeyboardFocusPropertyId {
            // SAFETY: The STA's focus observation cannot outlive the exact child window.
            return Ok(VARIANT::from(!self.container && unsafe { GetFocus() } == self.shared.hwnd));
        }
        if id == UIA_IsEnabledPropertyId {
            // SAFETY: Only the checked live child is observed.
            return Ok(VARIANT::from(unsafe { IsWindowEnabled(self.shared.hwnd) }.as_bool()));
        }
        if self.container {
            return Ok(if id == UIA_IsControlElementPropertyId {
                VARIANT::from(true)
            } else if id == UIA_IsContentElementPropertyId
                || id == UIA_IsKeyboardFocusablePropertyId
            {
                VARIANT::from(false)
            } else {
                VARIANT::default()
            });
        }
        let provider: IRawElementProviderSimple = self.native.get()?;
        // SAFETY: Native RichEdit owns the actual remaining text properties.
        let value = unsafe { provider.GetPropertyValue(id) }?;
        arrays::property(value)
    }
    fn navigate(&self, direction: NavigateDirection) -> windows::core::Result<*mut c_void> {
        self.shared.check()?;
        if self.container
            && (direction == NavigateDirection_FirstChild
                || direction == NavigateDirection_LastChild)
        {
            return Ok(arrays::interface(
                self.shared.element::<IRawElementProviderFragment>(false)?,
            ));
        }
        if !self.container && direction == NavigateDirection_Parent {
            return Ok(arrays::interface(
                self.shared.element::<IRawElementProviderFragment>(true)?,
            ));
        }
        Ok(std::ptr::null_mut())
    }
    fn focus(&self) -> windows::core::Result<()> {
        self.shared.check()?;
        // SAFETY: Setting focus targets the checked UI-thread native child only.
        let previous = unsafe { SetFocus(Some(self.shared.hwnd)) };
        // SAFETY: A null previous window can accompany a successful focus assignment.
        if unsafe { GetFocus() } == self.shared.hwnd {
            Ok(())
        } else {
            previous.map(|_| ()).and(Err(windows::Win32::Foundation::E_FAIL.into()))
        }
    }
}

macro_rules! element_interfaces {
    ($owner:ty) => {
        impl abi::Simple_Impl for $owner {
            unsafe fn provider_options(&self, out: *mut ProviderOptions) -> HRESULT {
                // SAFETY: Every output is initialized through the checked ABI helper.
                unsafe {
                    arrays::output(out, || {
                        self.element.shared.check()?;
                        Ok(ProviderOptions_ServerSideProvider
                            | ProviderOptions_UseComThreading
                            | ProviderOptions_ProviderOwnsSetFocus)
                    })
                }
            }
            unsafe fn get_pattern_provider(
                &self,
                id: UIA_PATTERN_ID,
                out: *mut *mut c_void,
            ) -> HRESULT {
                // SAFETY: Nullable output receives only guarded pattern interfaces.
                unsafe { arrays::output(out, || self.element.pattern(id)) }
            }
            unsafe fn get_property_value(&self, id: UIA_PROPERTY_ID, out: *mut VARIANT) -> HRESULT {
                // SAFETY: The initialized output adopts the owned returned VARIANT.
                unsafe { arrays::output(out, || self.element.property(id)) }
            }
            unsafe fn host_raw_element_provider(&self, out: *mut *mut c_void) -> HRESULT {
                // SAFETY: Only the HWND container returns its native window host provider.
                unsafe {
                    arrays::output(out, || {
                        self.element.shared.check()?;
                        if self.element.container {
                            Ok(UiaHostProviderFromHwnd(self.element.shared.hwnd)?.into_raw())
                        } else {
                            Ok(std::ptr::null_mut())
                        }
                    })
                }
            }
        }
        impl abi::Fragment_Impl for $owner {
            unsafe fn navigate(
                &self,
                direction: NavigateDirection,
                out: *mut *mut c_void,
            ) -> HRESULT {
                // SAFETY: The checked fragment tree returns owned interfaces or actual null.
                unsafe { arrays::output(out, || self.element.navigate(direction)) }
            }
            unsafe fn get_runtime_id(&self, out: *mut *mut SAFEARRAY) -> HRESULT {
                // SAFETY: A newly allocated runtime ID transfers to the COM caller.
                unsafe {
                    arrays::output(out, || {
                        self.element.shared.check()?;
                        if self.element.container {
                            Ok(std::ptr::null_mut())
                        } else {
                            arrays::runtime_id(true)
                        }
                    })
                }
            }
            unsafe fn bounding_rectangle(&self, out: *mut UiaRect) -> HRESULT {
                // SAFETY: The caller receives the current physical screen bounds.
                unsafe { arrays::output(out, || self.element.bounds()) }
            }
            unsafe fn get_embedded_fragment_roots(&self, out: *mut *mut SAFEARRAY) -> HRESULT {
                // SAFETY: Read-only log documents contain no embedded OLE documents.
                unsafe {
                    arrays::output(out, || {
                        self.element.shared.check()?;
                        Ok(std::ptr::null_mut())
                    })
                }
            }
            unsafe fn set_focus(&self) -> HRESULT {
                self.element.focus().into()
            }
            unsafe fn fragment_root(&self, out: *mut *mut c_void) -> HRESULT {
                // SAFETY: The retained HWND container is the actual fragment root.
                unsafe {
                    arrays::output(out, || {
                        Ok(arrays::interface(
                            self.element.shared.element::<IRawElementProviderFragmentRoot>(true)?,
                        ))
                    })
                }
            }
        }
    };
}
element_interfaces!(Container_Impl);
element_interfaces!(TextElement_Impl);

impl abi::Root_Impl for Container_Impl {
    unsafe fn element_provider_from_point(&self, x: f64, y: f64, out: *mut *mut c_void) -> HRESULT {
        // SAFETY: The output is initialized before checked native geometry or interface acquisition.
        unsafe {
            arrays::output(out, || {
                let bounds = self.element.bounds()?;
                if x >= bounds.left
                    && x < bounds.left + bounds.width
                    && y >= bounds.top
                    && y < bounds.top + bounds.height
                {
                    Ok(arrays::interface(
                        self.element.shared.element::<IRawElementProviderFragment>(false)?,
                    ))
                } else {
                    Ok(std::ptr::null_mut())
                }
            })
        }
    }
    unsafe fn get_focus(&self, out: *mut *mut c_void) -> HRESULT {
        // SAFETY: The checked STA focus determines whether the sole text fragment has focus.
        unsafe {
            arrays::output(out, || {
                self.element.shared.check()?;
                if GetFocus() == self.element.shared.hwnd {
                    Ok(arrays::interface(
                        self.element.shared.element::<IRawElementProviderFragment>(false)?,
                    ))
                } else {
                    Ok(std::ptr::null_mut())
                }
            })
        }
    }
}
impl abi::Site_Impl for WindowlessSite_Impl {
    unsafe fn get_adjacent_fragment(
        &self,
        direction: NavigateDirection,
        out: *mut *mut c_void,
    ) -> HRESULT {
        // SAFETY: The actual owned container answers the native text provider's parent navigation.
        unsafe {
            arrays::output(out, || {
                self.shared.check()?;
                if direction == NavigateDirection_Parent {
                    Ok(arrays::interface(self.shared.element::<IRawElementProviderFragment>(true)?))
                } else {
                    Ok(std::ptr::null_mut())
                }
            })
        }
    }
    unsafe fn get_runtime_id_prefix(&self, out: *mut *mut SAFEARRAY) -> HRESULT {
        // SAFETY: A freshly allocated prefix is transferred to the native provider.
        unsafe {
            arrays::output(out, || {
                self.shared.check()?;
                arrays::runtime_id(false)
            })
        }
    }
}
