// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Intercept every color-bearing render-target operation; preserve all native geometry.
use super::*;
use std::ffi::c_void;
use windows::{
    Win32::Graphics::{DirectWrite::*, Imaging::*},
    core::{BOOL, GUID, OutRef, PCWSTR, Result},
};
use windows_numerics::{Matrix3x2, Vector2};

// All forwarded pointers and borrowed interfaces originate in the synchronous native
// ID2D1RenderTarget callback. Their COM contracts retain them for the entire call.
macro_rules! forward {
    ($owner:expr, $method:ident $(, $argument:expr)*) => {{
        // SAFETY: The owned real target and callback arguments remain live until return.
        unsafe { (Interface::vtable(&$owner.shared.target).$method)(Interface::as_raw(&$owner.shared.target), $($argument),*) }
    }};
}
impl ID2D1Resource_Impl for Bridge_Impl {
    fn GetFactory(&self, factory: OutRef<'_, ID2D1Factory>) {
        // SAFETY: The real render target remains owned throughout the synchronous call.
        let result = unsafe { self.shared.target.GetFactory() };
        if let Err(cause) = result.and_then(|value| factory.write(Some(value))) {
            self.shared.fail_at("GetFactory", cause.code(), Detail::Native);
        }
    }
}
impl ID2D1RenderTarget_Impl for Bridge_Impl {
    fn CreateBitmap(
        &self,
        _size: &D2D_SIZE_U,
        _srcdata: *const c_void,
        _pitch: u32,
        _bitmapproperties: *const D2D1_BITMAP_PROPERTIES,
    ) -> Result<ID2D1Bitmap> {
        self.shared.unsupported("CreateBitmap")
    }
    fn CreateBitmapFromWicBitmap(
        &self,
        _wicbitmapsource: Ref<IWICBitmapSource>,
        _bitmapproperties: *const D2D1_BITMAP_PROPERTIES,
    ) -> Result<ID2D1Bitmap> {
        self.shared.unsupported("CreateBitmapFromWicBitmap")
    }
    fn CreateSharedBitmap(
        &self,
        _riid: *const GUID,
        _data: *mut c_void,
        _bitmapproperties: *const D2D1_BITMAP_PROPERTIES,
        _bitmap: OutRef<ID2D1Bitmap>,
    ) -> Result<()> {
        self.shared.unsupported("CreateSharedBitmap")
    }
    fn CreateBitmapBrush(
        &self,
        _bitmap: Ref<ID2D1Bitmap>,
        _bitmap_brushproperties: *const D2D1_BITMAP_BRUSH_PROPERTIES,
        _brushproperties: *const D2D1_BRUSH_PROPERTIES,
    ) -> Result<ID2D1BitmapBrush> {
        self.shared.unsupported("CreateBitmapBrush")
    }
    fn CreateSolidColorBrush(
        &self,
        color: *const D2D1_COLOR_F,
        brushproperties: *const D2D1_BRUSH_PROPERTIES,
    ) -> Result<ID2D1SolidColorBrush> {
        let mut brush = core::ptr::null_mut();
        self.shared.record(
            "CreateSolidColorBrush",
            forward!(self, CreateSolidColorBrush, color, brushproperties, &mut brush).ok(),
        )?;
        if brush.is_null() {
            return Err(self.shared.fail_at(
                "CreateSolidColorBrush",
                windows::Win32::Foundation::E_POINTER,
                Detail::Native,
            ));
        }
        // SAFETY: A successful COM creation transfers one owned reference to a checked nonnull output.
        Ok(unsafe { ID2D1SolidColorBrush::from_raw(brush) })
    }
    fn CreateGradientStopCollection(
        &self,
        _gradientstops: *const D2D1_GRADIENT_STOP,
        _gradientstopscount: u32,
        _colorinterpolationgamma: D2D1_GAMMA,
        _extendmode: D2D1_EXTEND_MODE,
    ) -> Result<ID2D1GradientStopCollection> {
        self.shared.unsupported("CreateGradientStopCollection")
    }
    fn CreateLinearGradientBrush(
        &self,
        _lineargradient_brushproperties: *const D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
        _brushproperties: *const D2D1_BRUSH_PROPERTIES,
        _gradientstopcollection: Ref<ID2D1GradientStopCollection>,
    ) -> Result<ID2D1LinearGradientBrush> {
        self.shared.unsupported("CreateLinearGradientBrush")
    }
    fn CreateRadialGradientBrush(
        &self,
        _radialgradient_brushproperties: *const D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES,
        _brushproperties: *const D2D1_BRUSH_PROPERTIES,
        _gradientstopcollection: Ref<ID2D1GradientStopCollection>,
    ) -> Result<ID2D1RadialGradientBrush> {
        self.shared.unsupported("CreateRadialGradientBrush")
    }
    fn CreateCompatibleRenderTarget(
        &self,
        _desiredsize: *const D2D_SIZE_F,
        _desiredpixelsize: *const D2D_SIZE_U,
        _desiredformat: *const D2D1_PIXEL_FORMAT,
        _options: D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS,
    ) -> Result<ID2D1BitmapRenderTarget> {
        self.shared.unsupported("CreateCompatibleRenderTarget")
    }
    fn CreateLayer(&self, size: *const D2D_SIZE_F) -> Result<ID2D1Layer> {
        let mut layer = core::ptr::null_mut();
        self.shared.record("CreateLayer", forward!(self, CreateLayer, size, &mut layer).ok())?;
        if layer.is_null() {
            return Err(self.shared.fail_at(
                "CreateLayer",
                windows::Win32::Foundation::E_POINTER,
                Detail::Native,
            ));
        }
        // SAFETY: A successful COM creation transfers one owned reference to a checked nonnull output.
        Ok(unsafe { ID2D1Layer::from_raw(layer) })
    }
    fn CreateMesh(&self) -> Result<ID2D1Mesh> {
        self.shared.unsupported("CreateMesh")
    }
    fn DrawLine(
        &self,
        point0: &Vector2,
        point1: &Vector2,
        brush: Ref<ID2D1Brush>,
        strokewidth: f32,
        strokestyle: Ref<ID2D1StrokeStyle>,
    ) {
        self.shared.paint("DrawLine", &brush, || {
            forward!(
                self,
                DrawLine,
                *point0,
                *point1,
                brush.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw),
                strokewidth,
                strokestyle.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw)
            );
        });
    }
    fn DrawRectangle(
        &self,
        rect: *const D2D_RECT_F,
        brush: Ref<ID2D1Brush>,
        strokewidth: f32,
        strokestyle: Ref<ID2D1StrokeStyle>,
    ) {
        self.shared.paint("DrawRectangle", &brush, || {
            forward!(
                self,
                DrawRectangle,
                rect,
                brush.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw),
                strokewidth,
                strokestyle.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw)
            );
        });
    }
    fn FillRectangle(&self, rect: *const D2D_RECT_F, brush: Ref<ID2D1Brush>) {
        self.shared.paint("FillRectangle", &brush, || {
            forward!(
                self,
                FillRectangle,
                rect,
                brush.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw)
            );
        });
    }
    fn DrawRoundedRectangle(
        &self,
        roundedrect: *const D2D1_ROUNDED_RECT,
        brush: Ref<ID2D1Brush>,
        strokewidth: f32,
        strokestyle: Ref<ID2D1StrokeStyle>,
    ) {
        self.shared.paint("DrawRoundedRectangle", &brush, || {
            forward!(
                self,
                DrawRoundedRectangle,
                roundedrect,
                brush.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw),
                strokewidth,
                strokestyle.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw)
            );
        });
    }
    fn FillRoundedRectangle(&self, roundedrect: *const D2D1_ROUNDED_RECT, brush: Ref<ID2D1Brush>) {
        self.shared.paint("FillRoundedRectangle", &brush, || {
            forward!(
                self,
                FillRoundedRectangle,
                roundedrect,
                brush.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw)
            );
        });
    }
    fn DrawEllipse(
        &self,
        ellipse: *const D2D1_ELLIPSE,
        brush: Ref<ID2D1Brush>,
        strokewidth: f32,
        strokestyle: Ref<ID2D1StrokeStyle>,
    ) {
        self.shared.paint("DrawEllipse", &brush, || {
            forward!(
                self,
                DrawEllipse,
                ellipse,
                brush.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw),
                strokewidth,
                strokestyle.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw)
            );
        });
    }
    fn FillEllipse(&self, ellipse: *const D2D1_ELLIPSE, brush: Ref<ID2D1Brush>) {
        self.shared.paint("FillEllipse", &brush, || {
            forward!(
                self,
                FillEllipse,
                ellipse,
                brush.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw)
            );
        });
    }
    fn DrawGeometry(
        &self,
        geometry: Ref<ID2D1Geometry>,
        brush: Ref<ID2D1Brush>,
        strokewidth: f32,
        strokestyle: Ref<ID2D1StrokeStyle>,
    ) {
        self.shared.paint("DrawGeometry", &brush, || {
            forward!(
                self,
                DrawGeometry,
                geometry.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw),
                brush.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw),
                strokewidth,
                strokestyle.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw)
            );
        });
    }
    fn FillGeometry(
        &self,
        geometry: Ref<ID2D1Geometry>,
        brush: Ref<ID2D1Brush>,
        opacitybrush: Ref<ID2D1Brush>,
    ) {
        if !opacitybrush.is_null() {
            self.shared.fail_at("FillGeometry", E_NOTIMPL, Detail::Unsupported);
            return;
        }
        self.shared.paint("FillGeometry", &brush, || {
            forward!(
                self,
                FillGeometry,
                geometry.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw),
                brush.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw),
                core::ptr::null_mut()
            );
        });
    }
    fn FillMesh(&self, mesh: Ref<ID2D1Mesh>, brush: Ref<ID2D1Brush>) {
        self.shared.paint("FillMesh", &brush, || {
            forward!(
                self,
                FillMesh,
                mesh.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw),
                brush.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw)
            );
        });
    }
    fn FillOpacityMask(
        &self,
        _opacitymask: Ref<ID2D1Bitmap>,
        _brush: Ref<ID2D1Brush>,
        _content: D2D1_OPACITY_MASK_CONTENT,
        _destinationrectangle: *const D2D_RECT_F,
        _sourcerectangle: *const D2D_RECT_F,
    ) {
        self.shared.fail_at("FillOpacityMask", E_NOTIMPL, Detail::Unsupported);
    }
    fn DrawBitmap(
        &self,
        _bitmap: Ref<ID2D1Bitmap>,
        _destinationrectangle: *const D2D_RECT_F,
        _opacity: f32,
        _interpolationmode: D2D1_BITMAP_INTERPOLATION_MODE,
        _sourcerectangle: *const D2D_RECT_F,
    ) {
        self.shared.fail_at("DrawBitmap", E_NOTIMPL, Detail::Unsupported);
    }
    fn DrawText(
        &self,
        string: &PCWSTR,
        stringlength: u32,
        textformat: Ref<IDWriteTextFormat>,
        layoutrect: *const D2D_RECT_F,
        defaultfillbrush: Ref<ID2D1Brush>,
        options: D2D1_DRAW_TEXT_OPTIONS,
        measuringmode: DWRITE_MEASURING_MODE,
    ) {
        self.shared.paint("DrawText", &defaultfillbrush, || {
            forward!(
                self,
                DrawText,
                *string,
                stringlength,
                textformat.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw),
                layoutrect,
                defaultfillbrush.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw),
                options,
                measuringmode
            );
        });
    }
    fn DrawTextLayout(
        &self,
        _origin: &Vector2,
        _textlayout: Ref<IDWriteTextLayout>,
        _defaultfillbrush: Ref<ID2D1Brush>,
        _options: D2D1_DRAW_TEXT_OPTIONS,
    ) {
        self.shared.fail_at("DrawTextLayout", E_NOTIMPL, Detail::Unsupported);
    }
    fn DrawGlyphRun(
        &self,
        baselineorigin: &Vector2,
        glyphrun: *const DWRITE_GLYPH_RUN,
        foregroundbrush: Ref<ID2D1Brush>,
        measuringmode: DWRITE_MEASURING_MODE,
    ) {
        let _scope = self.shared.glyph_scope(glyphrun, measuringmode);
        self.shared.paint("DrawGlyphRun", &foregroundbrush, || {
            if let Err(cause) = foregroundbrush.ok().and_then(|brush| {
                self.shared.glyphs(*baselineorigin, glyphrun, brush, measuringmode)
            }) {
                self.shared.fail_at("DrawGlyphRun", cause.code(), Detail::Native);
            }
        });
    }
    fn SetTransform(&self, transform: *const Matrix3x2) {
        forward!(self, SetTransform, transform);
    }
    fn GetTransform(&self, transform: *mut Matrix3x2) {
        forward!(self, GetTransform, transform);
    }
    fn SetAntialiasMode(&self, antialiasmode: D2D1_ANTIALIAS_MODE) {
        forward!(self, SetAntialiasMode, antialiasmode);
    }
    fn GetAntialiasMode(&self) -> D2D1_ANTIALIAS_MODE {
        forward!(self, GetAntialiasMode)
    }
    fn SetTextAntialiasMode(&self, textantialiasmode: D2D1_TEXT_ANTIALIAS_MODE) {
        forward!(self, SetTextAntialiasMode, textantialiasmode);
    }
    fn GetTextAntialiasMode(&self) -> D2D1_TEXT_ANTIALIAS_MODE {
        forward!(self, GetTextAntialiasMode)
    }
    fn SetTextRenderingParams(&self, textrenderingparams: Ref<IDWriteRenderingParams>) {
        forward!(
            self,
            SetTextRenderingParams,
            textrenderingparams.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw)
        );
    }
    fn GetTextRenderingParams(&self, textrenderingparams: OutRef<IDWriteRenderingParams>) {
        let mut pointer = core::ptr::null_mut();
        forward!(self, GetTextRenderingParams, &mut pointer);
        let value = if pointer.is_null() {
            None
        } else {
            // SAFETY: The native output transfers one owned reference; null means default parameters.
            Some(unsafe { IDWriteRenderingParams::from_raw(pointer) })
        };
        if let Err(cause) = textrenderingparams.write(value) {
            self.shared.fail_at("GetTextRenderingParams", cause.code(), Detail::Native);
        }
    }
    fn SetTags(&self, tag1: u64, tag2: u64) {
        forward!(self, SetTags, tag1, tag2);
    }
    fn GetTags(&self, tag1: *mut u64, tag2: *mut u64) {
        forward!(self, GetTags, tag1, tag2);
    }
    fn PushLayer(&self, layerparameters: *const D2D1_LAYER_PARAMETERS, layer: Ref<ID2D1Layer>) {
        if layerparameters.is_null() {
            self.shared.fail_at("PushLayer", windows::Win32::Foundation::E_POINTER, Detail::Native);
            return;
        }
        // SAFETY: COM supplies a live input structure throughout this callback.
        if unsafe { &*layerparameters }.opacityBrush.is_some() {
            self.shared.fail_at("PushLayer", E_NOTIMPL, Detail::Unsupported);
            return;
        }
        forward!(
            self,
            PushLayer,
            layerparameters,
            layer.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw)
        );
    }
    fn PopLayer(&self) {
        forward!(self, PopLayer);
    }
    fn Flush(&self, tag1: *mut u64, tag2: *mut u64) -> Result<()> {
        self.shared.record("Flush", forward!(self, Flush, tag1, tag2).ok())?;
        self.shared.failure.get().ok()
    }
    fn SaveDrawingState(&self, drawingstateblock: Ref<ID2D1DrawingStateBlock>) {
        forward!(
            self,
            SaveDrawingState,
            drawingstateblock.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw)
        );
    }
    fn RestoreDrawingState(&self, drawingstateblock: Ref<ID2D1DrawingStateBlock>) {
        forward!(
            self,
            RestoreDrawingState,
            drawingstateblock.as_ref().map_or(core::ptr::null_mut(), Interface::as_raw)
        );
    }
    fn PushAxisAlignedClip(&self, cliprect: *const D2D_RECT_F, antialiasmode: D2D1_ANTIALIAS_MODE) {
        forward!(self, PushAxisAlignedClip, cliprect, antialiasmode);
    }
    fn PopAxisAlignedClip(&self) {
        forward!(self, PopAxisAlignedClip);
    }
    fn Clear(&self, _clearcolor: *const D2D1_COLOR_F) {
        self.shared.fail_at("Clear", E_NOTIMPL, Detail::Unsupported);
    }
    fn BeginDraw(&self) {
        self.shared.fail_at("BeginDraw", E_NOTIMPL, Detail::Unsupported);
    }
    fn EndDraw(&self, _tag1: *mut u64, _tag2: *mut u64) -> Result<()> {
        self.shared.unsupported("EndDraw")
    }
    fn GetPixelFormat(&self) -> D2D1_PIXEL_FORMAT {
        // SAFETY: The owned target synchronously returns a value without borrowed storage.
        unsafe { self.shared.target.GetPixelFormat() }
    }
    fn SetDpi(&self, dpix: f32, dpiy: f32) {
        forward!(self, SetDpi, dpix, dpiy);
    }
    fn GetDpi(&self, dpix: *mut f32, dpiy: *mut f32) {
        forward!(self, GetDpi, dpix, dpiy);
    }
    fn GetSize(&self) -> D2D_SIZE_F {
        // SAFETY: The owned target synchronously returns a value without borrowed storage.
        unsafe { self.shared.target.GetSize() }
    }
    fn GetPixelSize(&self) -> D2D_SIZE_U {
        // SAFETY: The owned target synchronously returns a value without borrowed storage.
        unsafe { self.shared.target.GetPixelSize() }
    }
    fn GetMaximumBitmapSize(&self) -> u32 {
        forward!(self, GetMaximumBitmapSize)
    }
    fn IsSupported(&self, rendertargetproperties: *const D2D1_RENDER_TARGET_PROPERTIES) -> BOOL {
        forward!(self, IsSupported, rendertargetproperties)
    }
}
