// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Export the shown canvas output to other applications through one OLE copy drag.
//!
//! The canvas owner hands over the output's own filename and complete encoded bytes. They are
//! offered only as a virtual file: `FileGroupDescriptorW` names the original filename and
//! `FileContents` holds the bytes, so a drop keeps that name without exposing a Comfy, cache or
//! temporary path. OLE requires the drag source to run modally on the window's STA thread.

use super::*;
use crate::canvas::MainCanvasDragOut;
use std::mem::ManuallyDrop;
use windows::{
    Win32::{
        Foundation::{
            DATA_S_SAMEFORMATETC, DV_E_FORMATETC, DV_E_LINDEX, DV_E_TYMED, E_NOTIMPL,
            E_OUTOFMEMORY, E_POINTER, GlobalFree, HGLOBAL, OLE_E_ADVISENOTSUPPORTED,
        },
        System::{
            Com::{
                DATADIR_GET, DVASPECT_CONTENT, FORMATETC, IAdviseSink, IDataObject,
                IDataObject_Impl, IEnumFORMATETC, IEnumSTATDATA, STGMEDIUM, STGMEDIUM_0,
                TYMED_HGLOBAL,
            },
            DataExchange::RegisterClipboardFormatW,
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
            Ole::{
                DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, DoDragDrop, IDropSource,
                IDropSource_Impl,
            },
            SystemServices::{MK_LBUTTON, MK_RBUTTON, MODIFIERKEYS_FLAGS},
        },
        UI::Shell::{
            CFSTR_FILECONTENTS, CFSTR_FILEDESCRIPTORW, FD_FILESIZE, FD_PROGRESSUI, FILEDESCRIPTORW,
            FILEGROUPDESCRIPTORW, SHCreateStdEnumFmtEtc,
        },
    },
    core::{BOOL, HRESULT, Ref, implement},
};

/// `DRAGDROP_S_DROP`: the target accepted the data.
const DRAGDROP_S_DROP: HRESULT = HRESULT(0x0004_0100);
/// `DRAGDROP_S_CANCEL`: the user or source cancelled the drag.
const DRAGDROP_S_CANCEL: HRESULT = HRESULT(0x0004_0101);
/// `DRAGDROP_S_USEDEFAULTCURSORS`: OLE shows its standard drag cursors.
const DRAGDROP_S_USEDEFAULTCURSORS: HRESULT = HRESULT(0x0004_0102);
/// A file descriptor name holds 259 UTF-16 units and its terminator.
const NAME_UNITS: usize = 260;

/// Which virtual-file representation a format request names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Offer {
    /// The one-item `FileGroupDescriptorW`.
    Descriptor,
    /// The item's complete `FileContents`.
    Contents,
}

/// Clipboard format identities of one virtual file, registered once per drag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Formats {
    descriptor: u16,
    contents: u16,
}

impl Formats {
    /// Register the shell's virtual-file formats; registration returns existing identities.
    fn register() -> Result<Self, DesktopError> {
        // SAFETY: Both arguments are static NUL-terminated shell format names.
        let (descriptor, contents) = unsafe {
            (
                RegisterClipboardFormatW(CFSTR_FILEDESCRIPTORW),
                RegisterClipboardFormatW(CFSTR_FILECONTENTS),
            )
        };
        match (u16::try_from(descriptor), u16::try_from(contents)) {
            (Ok(descriptor), Ok(contents)) if descriptor != 0 && contents != 0 => {
                Ok(Self { descriptor, contents })
            }
            _ => Err(DesktopError::message(
                "register_canvas_drag_formats",
                "Virtual file clipboard formats are unavailable",
            )),
        }
    }

    /// Name the representation `format` requests, or why it cannot be supplied.
    fn offer(self, format: &FORMATETC) -> Result<Offer, HRESULT> {
        let offer = if format.cfFormat == self.descriptor {
            Offer::Descriptor
        } else if format.cfFormat == self.contents {
            Offer::Contents
        } else {
            return Err(DV_E_FORMATETC);
        };
        if format.dwAspect != DVASPECT_CONTENT.0 {
            return Err(DV_E_FORMATETC);
        }
        if format.tymed & TYMED_HGLOBAL.0 as u32 == 0 {
            return Err(DV_E_TYMED);
        }
        // The descriptor ignores the index; contents are item zero of a one-item group.
        if offer == Offer::Contents && !matches!(format.lindex, -1 | 0) {
            return Err(DV_E_LINDEX);
        }
        Ok(offer)
    }

    /// The formats offered for enumeration, in preference order.
    fn enumerated(self) -> [FORMATETC; 2] {
        let format = |cf_format| FORMATETC {
            cfFormat: cf_format,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        [format(self.descriptor), FORMATETC { lindex: 0, ..format(self.contents) }]
    }
}

/// Encode an output filename leaf as a descriptor name, refusing any path or device syntax.
fn descriptor_name(filename: &str) -> Option<[u16; NAME_UNITS]> {
    if filename.is_empty()
        || matches!(filename, "." | "..")
        || filename.ends_with(['.', ' '])
        || filename.chars().any(|character| {
            character.is_control()
                || matches!(character, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
    {
        return None;
    }
    let mut name = [0_u16; NAME_UNITS];
    // The final unit stays zero as the descriptor's terminator.
    for (index, unit) in filename.encode_utf16().enumerate() {
        *name[..NAME_UNITS - 1].get_mut(index)? = unit;
    }
    Some(name)
}

/// One validated output offered for drops; the claim covers its bytes until the last release.
struct VirtualFile {
    formats: Formats,
    name: [u16; NAME_UNITS],
    bytes: Vec<u8>,
    _claim: sugarsubstitute_contracts::execution::MemoryClaim,
}

impl VirtualFile {
    /// Admit only a nonempty output whose filename leaf a descriptor can carry.
    fn new(drag: MainCanvasDragOut, formats: Formats) -> Result<Self, DesktopError> {
        let name = descriptor_name(&drag.filename).ok_or_else(|| {
            DesktopError::message(
                "canvas_drag_output",
                "Output filename cannot name a dropped file",
            )
        })?;
        if drag.bytes.is_empty() {
            return Err(DesktopError::message(
                "canvas_drag_output",
                "Output bytes are unavailable",
            ));
        }
        Ok(Self { formats, name, bytes: drag.bytes, _claim: drag.claim })
    }

    /// The one-item group naming this file and its exact size.
    fn descriptor(&self) -> FILEGROUPDESCRIPTORW {
        let size = self.bytes.len() as u64;
        FILEGROUPDESCRIPTORW {
            cItems: 1,
            fgd: [FILEDESCRIPTORW {
                dwFlags: (FD_FILESIZE.0 | FD_PROGRESSUI.0) as u32,
                nFileSizeHigh: (size >> 32) as u32,
                nFileSizeLow: size as u32,
                cFileName: self.name,
                ..Default::default()
            }],
        }
    }

    /// Copy one representation into a new movable global block that the receiver releases.
    fn medium(&self, offer: Offer) -> windows::core::Result<STGMEDIUM> {
        let descriptor = self.descriptor();
        let bytes: &[u8] = match offer {
            // SAFETY: The descriptor is plain initialized data borrowed for this copy only.
            Offer::Descriptor => unsafe {
                std::slice::from_raw_parts(
                    (&raw const descriptor).cast::<u8>(),
                    size_of::<FILEGROUPDESCRIPTORW>(),
                )
            },
            Offer::Contents => &self.bytes,
        };
        let global = global_copy(bytes)?;
        Ok(STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: global },
            pUnkForRelease: ManuallyDrop::new(None),
        })
    }
}

/// Allocate a movable global block holding exactly `bytes`.
fn global_copy(bytes: &[u8]) -> windows::core::Result<HGLOBAL> {
    // SAFETY: A movable allocation of the exact length; failure returns an error.
    let global = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }?;
    // SAFETY: The block was just allocated and is not shared until returned.
    let target = unsafe { GlobalLock(global) };
    if target.is_null() {
        // SAFETY: This owner still holds the only reference to the unlocked block.
        let _ = unsafe { GlobalFree(Some(global)) };
        return Err(E_OUTOFMEMORY.into());
    }
    // SAFETY: The locked block holds at least `bytes.len()` writable bytes and cannot overlap
    // the borrowed source.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), target.cast::<u8>(), bytes.len());
        // A zero return with no error only reports that the block is now unlocked.
        let _ = GlobalUnlock(global);
    }
    Ok(global)
}

/// OLE's data reference; never agile, because drop targets reach it through this STA.
#[implement(IDataObject, Agile = false)]
struct Data(Rc<VirtualFile>);

impl IDataObject_Impl for Data_Impl {
    fn GetData(&self, format: *const FORMATETC) -> windows::core::Result<STGMEDIUM> {
        // SAFETY: OLE lends a live FORMATETC for this call, or a null pointer.
        let format = unsafe { format.as_ref() }.ok_or(E_POINTER)?;
        let offer = self.0.formats.offer(format).map_err(windows::core::Error::from)?;
        self.0.medium(offer)
    }

    fn GetDataHere(
        &self,
        _format: *const FORMATETC,
        _medium: *mut STGMEDIUM,
    ) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn QueryGetData(&self, format: *const FORMATETC) -> HRESULT {
        // SAFETY: OLE lends a live FORMATETC for this call, or a null pointer.
        match unsafe { format.as_ref() } {
            Some(format) => self.0.formats.offer(format).map_or_else(|code| code, |_| HRESULT(0)),
            None => E_POINTER,
        }
    }

    fn GetCanonicalFormatEtc(&self, _input: *const FORMATETC, output: *mut FORMATETC) -> HRESULT {
        if output.is_null() {
            return E_POINTER;
        }
        // SAFETY: OLE lends a writable FORMATETC; a null device means the same format applies.
        unsafe { (*output).ptd = std::ptr::null_mut() };
        DATA_S_SAMEFORMATETC
    }

    fn SetData(
        &self,
        _format: *const FORMATETC,
        _medium: *const STGMEDIUM,
        _release: BOOL,
    ) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn EnumFormatEtc(&self, direction: u32) -> windows::core::Result<IEnumFORMATETC> {
        if direction != DATADIR_GET.0 as u32 {
            return Err(E_NOTIMPL.into());
        }
        // SAFETY: The shell copies the borrowed format array into its own enumerator.
        unsafe { SHCreateStdEnumFmtEtc(&self.0.formats.enumerated()) }
    }

    fn DAdvise(
        &self,
        _format: *const FORMATETC,
        _flags: u32,
        _sink: Ref<IAdviseSink>,
    ) -> windows::core::Result<u32> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }

    fn DUnadvise(&self, _connection: u32) -> windows::core::Result<()> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }

    fn EnumDAdvise(&self) -> windows::core::Result<IEnumSTATDATA> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }
}

/// Continue while the primary button is held; Escape or a secondary press cancels.
fn continuation(escape: bool, keys: MODIFIERKEYS_FLAGS) -> HRESULT {
    if escape || keys.0 & MK_RBUTTON.0 != 0 {
        DRAGDROP_S_CANCEL
    } else if keys.0 & MK_LBUTTON.0 == 0 {
        DRAGDROP_S_DROP
    } else {
        HRESULT(0)
    }
}

/// OLE's drag source; the standard cursors show the effect each target offers.
#[implement(IDropSource, Agile = false)]
struct Source;

impl IDropSource_Impl for Source_Impl {
    fn QueryContinueDrag(&self, escape: BOOL, keys: MODIFIERKEYS_FLAGS) -> HRESULT {
        continuation(escape.as_bool(), keys)
    }

    fn GiveFeedback(&self, _effect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

/// Build the data object offered for `drag`.
fn data_object(drag: MainCanvasDragOut) -> Result<IDataObject, DesktopError> {
    Ok(Data(Rc::new(VirtualFile::new(drag, Formats::register()?)?)).into())
}

/// Run one modal copy drag on the window's OLE-initialized UI thread.
///
/// Returns the effect the drop target chose; a cancelled or refused drag returns
/// `DROPEFFECT_NONE`. A target may keep the data object, and with it the bytes and their
/// claim, until it finishes reading.
pub(super) fn run(drag: MainCanvasDragOut) -> Result<DROPEFFECT, DesktopError> {
    let data = data_object(drag)?;
    let source: IDropSource = Source.into();
    let mut effect = DROPEFFECT_NONE;
    // SAFETY: Both COM objects belong to this STA, which initialized OLE for the window's
    // drop registration; `effect` is live local storage for the modal call.
    let result = unsafe { DoDragDrop(&data, &source, DROPEFFECT_COPY, &raw mut effect) };
    match result {
        DRAGDROP_S_DROP => Ok(DROPEFFECT(effect.0 & DROPEFFECT_COPY.0)),
        DRAGDROP_S_CANCEL => Ok(DROPEFFECT_NONE),
        code => Err(DesktopError::new("run_canvas_drag_out", windows::core::Error::from(code))),
    }
}

#[cfg(test)]
mod tests;
