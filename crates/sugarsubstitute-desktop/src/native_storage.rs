// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fund independent native pixel copies from accepted source frames without pinning obsolete preparation.

use crate::DesktopError;
use sugarsubstitute_contracts::{execution::MemoryClaim, presentation::PreparedFrame};

#[cfg(any(target_os = "linux", test))]
use sugarsubstitute_contracts::presentation::FrameIdentity;

/// Retain one platform image and the full identity of its last successful pixel copy.
#[cfg(any(target_os = "linux", test))]
pub(crate) struct CopiedImage<T> {
    image: Option<(FrameIdentity, T)>,
}

#[cfg(any(target_os = "linux", test))]
impl<T> Default for CopiedImage<T> {
    /// Begin without native copied storage or an accepted identity.
    fn default() -> Self {
        Self { image: None }
    }
}

#[cfg(any(target_os = "linux", test))]
impl<T> CopiedImage<T> {
    /// Reuse identical exposure; replace image/identity together only after successful copying.
    pub(crate) fn prepare(
        &mut self,
        frame: &PreparedFrame,
        copy: impl FnOnce(&PreparedFrame) -> Result<T, DesktopError>,
    ) -> Result<(), DesktopError> {
        if self.image.as_ref().is_some_and(|(identity, _)| *identity == frame.identity()) {
            return Ok(());
        }
        let image = copy(frame)?;
        self.image = Some((frame.identity(), image));
        Ok(())
    }

    /// Borrow retained native storage without admitting another allocation or duplicating its claim.
    pub(crate) fn image(&self) -> Option<&T> {
        self.image.as_ref().map(|(_, image)| image)
    }
}

/// Retain Cairo's copied byte allocation before its non-cloneable admission claim.
#[cfg(any(target_os = "linux", test))]
pub(crate) struct NativeBytes {
    bytes: Vec<u8>,
    _claim: MemoryClaim,
}

#[cfg(any(target_os = "linux", test))]
impl NativeBytes {
    /// Claim before allocation and reject allocator capacity exceeding that exact charge.
    pub(crate) fn new(frame: &PreparedFrame) -> Result<Self, DesktopError> {
        let length = frame.shape().byte_len();
        let claim = frame
            .claim_storage(length as u64)
            .map_err(|error| DesktopError::new("claim_native_bytes", error))?;
        // Locals and fields release pixel storage before returning its claim capacity.
        let mut bytes = allocate_bytes(length)?;
        if bytes.capacity() != length {
            return Err(DesktopError::message(
                "allocate_native_bytes",
                "Allocator capacity differs from the preclaimed native byte length",
            ));
        }
        for (rgba, native) in frame.pixels().chunks_exact(4).zip(bytes.chunks_exact_mut(4)) {
            let argb = (u32::from(rgba[3]) << 24)
                | (u32::from(rgba[0]) << 16)
                | (u32::from(rgba[1]) << 8)
                | u32::from(rgba[2]);
            native.copy_from_slice(&argb.to_ne_bytes());
        }
        Ok(Self { bytes, _claim: claim })
    }
}

#[cfg(any(target_os = "linux", test))]
impl AsMut<[u8]> for NativeBytes {
    /// Lend Cairo the retained top-down native ARGB bytes without copying or changing capacity.
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
}

/// Use fallible exact reservation rather than an infallible allocation on the native thread.
#[cfg(any(target_os = "linux", test))]
fn allocate_bytes(length: usize) -> Result<Vec<u8>, DesktopError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|error| DesktopError::new("allocate_native_bytes", error))?;
    bytes.resize(length, 0);
    Ok(bytes)
}

#[cfg(target_os = "windows")]
use std::ffi::c_void;
#[cfg(target_os = "windows")]
use sugarsubstitute_contracts::presentation::FrameShape;
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, HBITMAP, HDC, HGDIOBJ, SelectObject,
};

/// Own one selected top-down DIB and its admitted storage credit on the native thread.
#[cfg(target_os = "windows")]
pub(crate) struct Dib {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    pixels: *mut c_void,
    shape: FrameShape,
    claim: Option<MemoryClaim>,
}

#[cfg(target_os = "windows")]
impl Dib {
    /// Reuse same-sized storage; construct replacements fully before releasing the previous DIB.
    pub(crate) fn for_frame<'a>(
        retained: &'a mut Option<Self>,
        frame: &PreparedFrame,
    ) -> Result<&'a mut Self, DesktopError> {
        Self::for_shape(retained, frame, frame.shape())
    }

    /// Allocate only the requested native region using the retained frame's admitted storage.
    pub(crate) fn for_shape<'a>(
        retained: &'a mut Option<Self>,
        frame: &PreparedFrame,
        shape: FrameShape,
    ) -> Result<&'a mut Self, DesktopError> {
        if shape.width() > frame.width() || shape.height() > frame.height() {
            return Err(DesktopError::message(
                "allocate_dib_region",
                "native region exceeds its source frame",
            ));
        }
        if retained.as_ref().is_none_or(|dib| {
            !dib.live() || dib.width() != shape.width() || dib.height() != shape.height()
        }) {
            let replacement = Self::new(frame, shape)?;
            if let Some(old) = retained {
                old.close()?;
            }
            *retained = Some(replacement);
        }
        let dib = retained.as_mut().ok_or_else(|| {
            DesktopError::message("allocate_dib", "DIB allocation did not retain a surface")
        })?;
        frame
            .fund_storage(
                dib.claim.as_mut().ok_or_else(|| {
                    DesktopError::message("fund_dib", "Native storage credit absent")
                })?,
            )
            .map_err(|cause| DesktopError::new("fund_dib", cause))?;
        Ok(dib)
    }

    /// Claim the complete physical pixel storage before the first native allocation.
    fn new(frame: &PreparedFrame, shape: FrameShape) -> Result<Self, DesktopError> {
        let claim = frame
            .claim_storage(shape.byte_len() as u64)
            .map_err(|error| DesktopError::new("claim_dib_storage", error))?;
        let mut dib = Self {
            dc: HDC::default(),
            bitmap: HBITMAP::default(),
            previous: HGDIOBJ::default(),
            pixels: std::ptr::null_mut(),
            shape,
            claim: Some(claim),
        };
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                // FrameShape bounds dimensions to 4096 and byte length to a valid slice.
                biWidth: shape.width() as i32,
                biHeight: -(shape.height() as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        // SAFETY: The bounded descriptor owns top-down 32-bit storage. Partial state
        // immediately belongs to dib, whose Drop preserves cleanup and charge ordering.
        unsafe {
            dib.dc = CreateCompatibleDC(None);
            if dib.dc.is_invalid() {
                return Err(DesktopError::message("allocate_dib_dc", "CreateCompatibleDC failed"));
            }
            dib.bitmap =
                CreateDIBSection(Some(dib.dc), &info, DIB_RGB_COLORS, &mut dib.pixels, None, 0)
                    .map_err(|error| DesktopError::new("allocate_dib", error))?;
            dib.previous = SelectObject(dib.dc, dib.bitmap.into());
        }
        if !valid_selection(dib.previous) || dib.pixels.is_null() {
            return Err(DesktopError::message("select_dib", "DIB selection or storage is invalid"));
        }
        Ok(dib)
    }

    /// Borrow complete pixel storage; validated dimensions prevent pointer-slice overflow.
    pub(crate) fn pixels_mut(&mut self) -> Result<&mut [u8], DesktopError> {
        if !self.live() {
            return Err(DesktopError::message(
                "borrow_dib_pixels",
                "Native DIB storage is closed or partially released",
            ));
        }
        // SAFETY: A successfully constructed DIB owns exactly shape.byte_len bytes,
        // remains on its creating native thread, and is exclusively borrowed here.
        Ok(unsafe { std::slice::from_raw_parts_mut(self.pixels.cast(), self.shape.byte_len()) })
    }

    /// Return the bounded device-pixel width of the retained allocation.
    pub(crate) fn width(&self) -> u32 {
        self.shape.width()
    }
    /// Return the bounded device-pixel height of the retained allocation.
    pub(crate) fn height(&self) -> u32 {
        self.shape.height()
    }
    /// Borrow the DC selected into this live native allocation.
    pub(crate) fn dc(&self) -> HDC {
        self.dc
    }

    /// Report native destruction failures while retaining every unreleased handle and its charge.
    pub(crate) fn close(&mut self) -> Result<(), DesktopError> {
        let mut failure = None;
        // SAFETY: Every non-invalid handle remains owned here. Successful calls clear
        // ownership immediately, so explicit close and Drop cannot destroy it twice.
        unsafe {
            if valid_selection(self.previous) && !self.dc.is_invalid() {
                if valid_selection(SelectObject(self.dc, self.previous)) {
                    self.previous = HGDIOBJ::default();
                } else {
                    DesktopError::retain_first(
                        &mut failure,
                        DesktopError::message(
                            "restore_dib_selection",
                            "SelectObject failed during native storage teardown",
                        ),
                    );
                }
            }
            if !self.bitmap.is_invalid() {
                self.delete_bitmap(&mut failure);
            }
            if !self.dc.is_invalid() {
                match DeleteDC(self.dc).ok() {
                    Ok(()) => {
                        self.dc = HDC::default();
                        self.previous = HGDIOBJ::default();
                    }
                    Err(error) => DesktopError::retain_first(
                        &mut failure,
                        DesktopError::new("release_dib_dc", error),
                    ),
                }
            }
            if !self.bitmap.is_invalid() && self.dc.is_invalid() {
                // Deleting the DC detaches a failed selection before final bitmap release.
                self.delete_bitmap(&mut failure);
            }
        }
        if self.bitmap.is_invalid() && self.dc.is_invalid() {
            self.claim = None;
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Determine whether the allocation can still accept native presentation after partial cleanup.
    fn live(&self) -> bool {
        !self.dc.is_invalid()
            && !self.bitmap.is_invalid()
            && !self.pixels.is_null()
            && valid_selection(self.previous)
    }

    /// Transfer bitmap ownership only on confirmed release; callers retain the native handles.
    unsafe fn delete_bitmap(&mut self, failure: &mut Option<DesktopError>) {
        // SAFETY: Callers own this bitmap and either restored selection or destroyed its DC.
        match unsafe { DeleteObject(self.bitmap.into()) }.ok() {
            Ok(()) => {
                self.bitmap = HBITMAP::default();
                self.pixels = std::ptr::null_mut();
            }
            Err(error) => {
                DesktopError::retain_first(failure, DesktopError::new("release_dib", error))
            }
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for Dib {
    /// Attempt remaining owned cleanup; surviving storage cannot return its admitted bytes.
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            tracing::error!(operation = "drop_native_storage", cause = %error, "Native storage fallback teardown failed");
        }
        if let Some(claim) = self.claim.take() {
            tracing::error!(
                operation = "retain_failed_dib_charge",
                bytes = claim.bytes(),
                "Native resources survived teardown; retaining their original admitted charge"
            );
            std::mem::forget(claim);
        }
    }
}

/// Treat both documented GDI selection failure sentinels as invalid.
#[cfg(target_os = "windows")]
fn valid_selection(object: HGDIOBJ) -> bool {
    !object.is_invalid() && object.0 as isize != -1
}

#[cfg(test)]
#[path = "native_storage_tests.rs"]
mod tests;
