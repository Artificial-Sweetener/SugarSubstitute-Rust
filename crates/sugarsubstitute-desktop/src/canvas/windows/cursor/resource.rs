// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Turn bundled CUR frames and rendered cursor images into private Windows cursors.
//!
//! Both paths hand Windows the same thing: a hotspot prefix followed by one 32-bit
//! bottom-up DIB with its AND mask, which `CreateIconFromResourceEx` copies synchronously.

use super::*;
use ::windows::Win32::UI::WindowsAndMessaging::{CreateIconFromResourceEx, LR_DEFAULTCOLOR};
use sugarsubstitute_ui::cursors::CursorImage;

const CURSOR_RESOURCE_VERSION: u32 = 0x0003_0000;
const CURSOR_ENTRIES: usize = 3;
const DIRECTORY_BYTES: usize = 6 + CURSOR_ENTRIES * 16;
const DIB_HEADER_BYTES: usize = 40;

/// Create a private cursor from the nearest frame of a bundled three-size CUR archive, scaled
/// by Windows to the requested native cursor size.
pub(super) fn load_cursor(bytes: &[u8], size: [i32; 2]) -> Result<OwnedCursor, Box<dyn Error>> {
    let frame = select_frame(bytes, size)?;
    create(frame.hotspot, frame.bytes, size)
}

/// Create a private cursor of exactly the rendered image's size and hotspot.
pub(super) fn rendered_cursor(image: &CursorImage) -> Result<OwnedCursor, Box<dyn Error>> {
    let extent = [image.width(), image.height()];
    let size = extent.map(|edge| i32::try_from(edge).unwrap_or(i32::MAX));
    let hotspot = image.hotspot().map(|value| u16::try_from(value).unwrap_or(u16::MAX));
    if extent.iter().any(|edge| *edge == 0 || *edge > u32::from(u16::MAX))
        || hotspot[0] as u32 >= extent[0]
        || hotspot[1] as u32 >= extent[1]
    {
        return Err(DesktopError::message(
            "canvas_cursor_render",
            "Rendered cursor geometry is unsupported",
        )
        .into());
    }
    create(hotspot, &dib(image)?, size)
}

fn create(hotspot: [u16; 2], dib: &[u8], size: [i32; 2]) -> Result<OwnedCursor, Box<dyn Error>> {
    let mut resource = Vec::new();
    resource
        .try_reserve_exact(dib.len() + 4)
        .map_err(|cause| DesktopError::new("canvas_cursor_resource", cause))?;
    resource.extend_from_slice(&hotspot[0].to_le_bytes());
    resource.extend_from_slice(&hotspot[1].to_le_bytes());
    resource.extend_from_slice(dib);
    // SAFETY: The resource holds a hotspot prefix and a complete 32-bit cursor DIB, either proven
    // by the bounded archive parser or synthesized here. Windows copies it synchronously and
    // returns a private cursor at the requested native size.
    let icon = unsafe {
        CreateIconFromResourceEx(
            &resource,
            false,
            CURSOR_RESOURCE_VERSION,
            size[0],
            size[1],
            LR_DEFAULTCOLOR,
        )?
    };
    Ok(OwnedCursor(HCURSOR(icon.0)))
}

/// One straight-alpha BGRA bottom-up color plane plus the legacy AND mask, which marks fully
/// transparent pixels for consumers that ignore alpha.
fn dib(image: &CursorImage) -> Result<Vec<u8>, DesktopError> {
    let [width, height] = [image.width() as usize, image.height() as usize];
    let mask_stride = width.div_ceil(32) * 4;
    let color = width * height * 4;
    let mask = mask_stride * height;
    let header =
        [40_u32.to_le_bytes(), (width as u32).to_le_bytes(), (2 * height as u32).to_le_bytes()];
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(DIB_HEADER_BYTES + color + mask)
        .map_err(|cause| DesktopError::new("canvas_cursor_resource", cause))?;
    header.iter().for_each(|field| bytes.extend_from_slice(field));
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&32_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&((color + mask) as u32).to_le_bytes());
    bytes.extend_from_slice(&[0; 16]);
    let rgba = image.rgba();
    for row in (0..height).rev() {
        for pixel in rgba[row * width * 4..(row + 1) * width * 4].chunks_exact(4) {
            bytes.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }
    for row in (0..height).rev() {
        let start = bytes.len();
        bytes.resize(start + mask_stride, 0);
        for column in 0..width {
            if rgba[(row * width + column) * 4 + 3] == 0 {
                bytes[start + column / 8] |= 0x80 >> (column % 8);
            }
        }
    }
    Ok(bytes)
}

#[derive(Clone, Copy)]
pub(super) struct CursorFrame<'a> {
    pub(super) extent: [u16; 2],
    pub(super) hotspot: [u16; 2],
    pub(super) bytes: &'a [u8],
}

pub(super) fn select_frame(
    bytes: &[u8],
    requested: [i32; 2],
) -> Result<CursorFrame<'_>, DesktopError> {
    if bytes.len() < DIRECTORY_BYTES
        || word(bytes, 0) != Some(0)
        || word(bytes, 2) != Some(2)
        || word(bytes, 4) != Some(CURSOR_ENTRIES as u16)
        || requested.iter().any(|edge| *edge <= 0)
    {
        return Err(invalid_cursor());
    }
    let mut selected = None;
    let mut selected_distance = u64::MAX;
    for index in 0..CURSOR_ENTRIES {
        let entry = 6 + index * 16;
        let width = extent_byte(bytes[entry]);
        let height = extent_byte(bytes[entry + 1]);
        if bytes[entry + 2] != 0 || bytes[entry + 3] != 0 {
            return Err(invalid_cursor());
        }
        let hotspot = [
            word(bytes, entry + 4).ok_or_else(invalid_cursor)?,
            word(bytes, entry + 6).ok_or_else(invalid_cursor)?,
        ];
        let length = usize::try_from(dword(bytes, entry + 8).ok_or_else(invalid_cursor)?)
            .map_err(|_| invalid_cursor())?;
        let offset = usize::try_from(dword(bytes, entry + 12).ok_or_else(invalid_cursor)?)
            .map_err(|_| invalid_cursor())?;
        let end = offset.checked_add(length).ok_or_else(invalid_cursor)?;
        if offset < DIRECTORY_BYTES || length == 0 || end > bytes.len() {
            return Err(invalid_cursor());
        }
        let frame = CursorFrame { extent: [width, height], hotspot, bytes: &bytes[offset..end] };
        validate_dib(frame)?;
        let requested = [
            requested[0].min(i32::from(u16::MAX)) as u16,
            requested[1].min(i32::from(u16::MAX)) as u16,
        ];
        let distance =
            u64::from(width.abs_diff(requested[0])) + u64::from(height.abs_diff(requested[1]));
        let prefer_larger = selected.is_some_and(|current: CursorFrame<'_>| {
            distance == selected_distance && width > current.extent[0]
        });
        if distance < selected_distance || prefer_larger {
            selected = Some(frame);
            selected_distance = distance;
        }
    }
    selected.ok_or_else(invalid_cursor)
}

fn validate_dib(frame: CursorFrame<'_>) -> Result<(), DesktopError> {
    let header = dword(frame.bytes, 0).ok_or_else(invalid_cursor)?;
    let width = dword(frame.bytes, 4).ok_or_else(invalid_cursor)?;
    let height = dword(frame.bytes, 8).ok_or_else(invalid_cursor)?;
    let planes = word(frame.bytes, 12).ok_or_else(invalid_cursor)?;
    let bits = word(frame.bytes, 14).ok_or_else(invalid_cursor)?;
    let compression = dword(frame.bytes, 16).ok_or_else(invalid_cursor)?;
    let xor = usize::from(frame.extent[0])
        .checked_mul(usize::from(frame.extent[1]))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(invalid_cursor)?;
    let mask_stride = usize::from(frame.extent[0]).div_ceil(32) * 4;
    let required = DIB_HEADER_BYTES
        .checked_add(xor)
        .and_then(|bytes| bytes.checked_add(mask_stride * usize::from(frame.extent[1])))
        .ok_or_else(invalid_cursor)?;
    if header != 40
        || width != u32::from(frame.extent[0])
        || height != u32::from(frame.extent[1]) * 2
        || planes != 1
        || bits != 32
        || compression != 0
        || frame.hotspot[0] >= frame.extent[0]
        || frame.hotspot[1] >= frame.extent[1]
        || frame.bytes.len() < required
    {
        return Err(invalid_cursor());
    }
    Ok(())
}

const fn extent_byte(value: u8) -> u16 {
    if value == 0 { 256 } else { value as u16 }
}

fn word(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*bytes.get(offset)?, *bytes.get(offset + 1)?]))
}

fn dword(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
        *bytes.get(offset + 2)?,
        *bytes.get(offset + 3)?,
    ]))
}

fn invalid_cursor() -> DesktopError {
    DesktopError::message("canvas_cursor_asset", "Embedded Qt cursor resource is invalid")
}
