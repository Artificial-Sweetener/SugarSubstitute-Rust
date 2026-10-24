// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Nullable COM outputs retain their native ABI instead of manufacturing null Rust interfaces.
use super::*;
use std::mem::ManuallyDrop;
use windows_core::{IUnknown, IUnknown_Vtbl, interface};

/// IRawElementProviderSimple preserves nullable outputs for unsupported patterns and host providers.
#[interface("d6dd68d1-86fd-4332-8666-9abedea2d24c")]
pub(super) unsafe trait Simple: IUnknown {
    fn provider_options(&self, out: *mut ProviderOptions) -> HRESULT;
    fn get_pattern_provider(&self, id: UIA_PATTERN_ID, out: *mut *mut c_void) -> HRESULT;
    fn get_property_value(&self, id: UIA_PROPERTY_ID, out: *mut VARIANT) -> HRESULT;
    fn host_raw_element_provider(&self, out: *mut *mut c_void) -> HRESULT;
}
/// IRawElementProviderFragment preserves successful null navigation at the edge of a fragment.
#[interface("f7063da8-8359-439c-9297-bbc5299a7d87")]
pub(super) unsafe trait Fragment: IUnknown {
    fn navigate(&self, direction: NavigateDirection, out: *mut *mut c_void) -> HRESULT;
    fn get_runtime_id(&self, out: *mut *mut SAFEARRAY) -> HRESULT;
    fn bounding_rectangle(&self, out: *mut UiaRect) -> HRESULT;
    fn get_embedded_fragment_roots(&self, out: *mut *mut SAFEARRAY) -> HRESULT;
    fn set_focus(&self) -> HRESULT;
    fn fragment_root(&self, out: *mut *mut c_void) -> HRESULT;
}
/// IRawElementProviderFragmentRoot reports the actual owned child or successful absence.
#[interface("620ce2a5-ab8f-40a9-86cb-de3c75599b58")]
pub(super) unsafe trait Root: IUnknown {
    fn element_provider_from_point(&self, x: f64, y: f64, out: *mut *mut c_void) -> HRESULT;
    fn get_focus(&self, out: *mut *mut c_void) -> HRESULT;
}
/// IRawElementProviderWindowlessSite connects the native text fragment to its real HWND container.
#[interface("0a2a93cc-bfad-42ac-9b2e-0991fb0d3ea0")]
pub(super) unsafe trait Site: IUnknown {
    fn get_adjacent_fragment(&self, direction: NavigateDirection, out: *mut *mut c_void)
    -> HRESULT;
    fn get_runtime_id_prefix(&self, out: *mut *mut SAFEARRAY) -> HRESULT;
}
/// ITextProvider returns guarded native document ranges and selection arrays.
#[interface("3589c92c-63f3-4367-99bb-ada653b77cf2")]
pub(super) unsafe trait Text: IUnknown {
    fn get_selection(&self, out: *mut *mut SAFEARRAY) -> HRESULT;
    fn get_visible_ranges(&self, out: *mut *mut SAFEARRAY) -> HRESULT;
    fn range_from_child(&self, child: *mut c_void, out: *mut *mut c_void) -> HRESULT;
    fn range_from_point(&self, point: UiaPoint, out: *mut *mut c_void) -> HRESULT;
    fn document_range(&self, out: *mut *mut c_void) -> HRESULT;
    fn supported_text_selection(&self, out: *mut SupportedTextSelection) -> HRESULT;
}
/// ITextProvider2 retains nullable annotation/caret ranges without creating synthetic ranges.
#[interface("0dc5e6ed-3e16-4bf1-8f9a-a979878bc195")]
pub(super) unsafe trait Text2: Text {
    fn range_from_annotation(&self, annotation: *mut c_void, out: *mut *mut c_void) -> HRESULT;
    fn get_caret_range(&self, active: *mut BOOL, out: *mut *mut c_void) -> HRESULT;
}
/// ITextEditProvider preserves native composition and conversion range capabilities.
#[interface("ea3605b4-3a05-400e-b5f9-4e91b40f6176")]
pub(super) unsafe trait TextEdit: Text {
    fn get_active_composition(&self, out: *mut *mut c_void) -> HRESULT;
    fn get_conversion_target(&self, out: *mut *mut c_void) -> HRESULT;
}
/// ITextRangeProvider uses borrowed COM inputs and nullable owned results at the native ABI.
#[interface("5347ad7b-c355-46f8-aff5-909033582f63")]
pub(super) unsafe trait Range: IUnknown {
    fn clone_range(&self, out: *mut *mut c_void) -> HRESULT;
    fn compare(&self, range: *mut c_void, out: *mut BOOL) -> HRESULT;
    fn compare_endpoints(
        &self,
        endpoint: TextPatternRangeEndpoint,
        range: *mut c_void,
        target: TextPatternRangeEndpoint,
        out: *mut i32,
    ) -> HRESULT;
    fn expand_to_enclosing_unit(&self, unit: TextUnit) -> HRESULT;
    fn find_attribute(
        &self,
        id: UIA_TEXTATTRIBUTE_ID,
        value: ManuallyDrop<VARIANT>,
        backward: BOOL,
        out: *mut *mut c_void,
    ) -> HRESULT;
    fn find_text(
        &self,
        text: *mut c_void,
        backward: BOOL,
        ignore_case: BOOL,
        out: *mut *mut c_void,
    ) -> HRESULT;
    fn get_attribute_value(&self, id: UIA_TEXTATTRIBUTE_ID, out: *mut VARIANT) -> HRESULT;
    fn get_bounding_rectangles(&self, out: *mut *mut SAFEARRAY) -> HRESULT;
    fn get_enclosing_element(&self, out: *mut *mut c_void) -> HRESULT;
    fn get_text(&self, maximum: i32, out: *mut BSTR) -> HRESULT;
    fn r#move(&self, unit: TextUnit, count: i32, out: *mut i32) -> HRESULT;
    fn move_endpoint_by_unit(
        &self,
        endpoint: TextPatternRangeEndpoint,
        unit: TextUnit,
        count: i32,
        out: *mut i32,
    ) -> HRESULT;
    fn move_endpoint_by_range(
        &self,
        endpoint: TextPatternRangeEndpoint,
        range: *mut c_void,
        target: TextPatternRangeEndpoint,
    ) -> HRESULT;
    fn select(&self) -> HRESULT;
    fn add_to_selection(&self) -> HRESULT;
    fn remove_from_selection(&self) -> HRESULT;
    fn scroll_into_view(&self, top: BOOL) -> HRESULT;
    fn get_children(&self, out: *mut *mut SAFEARRAY) -> HRESULT;
}
/// ITextRangeProvider2 keeps the native range's context-menu operation behind the same gate.
#[interface("9bbce42c-1921-4f18-89ca-dba1910a0386")]
pub(super) unsafe trait Range2: Range {
    fn show_context_menu(&self) -> HRESULT;
}
