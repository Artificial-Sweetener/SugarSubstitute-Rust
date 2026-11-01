// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the drag-out data object through real OLE calls and the source's continuation rule.

use super::*;
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryLease, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};
use windows::Win32::System::{
    Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize},
    Memory::GlobalSize,
    Ole::ReleaseStgMedium,
    SystemServices::MK_CONTROL,
};

type TestResult = Result<(), Box<dyn Error>>;

const FORMATS: Formats = Formats { descriptor: 0xc101, contents: 0xc102 };

/// Count released admission so the test proves the claim lives exactly as long as the data.
struct Released(Arc<AtomicU64>);
impl MemoryRelease for Released {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::SeqCst);
    }
}

fn identity() -> Result<crate::MainIdentity, Box<dyn Error>> {
    Ok(crate::MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}

fn drag(
    filename: &str,
    bytes: Vec<u8>,
    released: &Arc<AtomicU64>,
) -> Result<MainCanvasDragOut, Box<dyn Error>> {
    let lease = MemoryLease::new(bytes.len() as u64, Box::new(Released(released.clone())));
    let claim = lease.claim(bytes.len() as u64)?;
    Ok(MainCanvasDragOut { identity: identity()?, filename: filename.into(), bytes, claim })
}

fn request(cf_format: u16, tymed: u32, lindex: i32) -> FORMATETC {
    FORMATETC {
        cfFormat: cf_format,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex,
        tymed,
    }
}

/// Copy a returned global block and release the medium as a drop target would.
fn take(mut medium: STGMEDIUM) -> Vec<u8> {
    assert_eq!(medium.tymed, TYMED_HGLOBAL.0 as u32);
    // SAFETY: The data object returned a movable global block owned by this medium.
    let bytes = unsafe {
        let global = medium.u.hGlobal;
        let size = GlobalSize(global);
        let source = GlobalLock(global);
        assert!(!source.is_null());
        let copied = std::slice::from_raw_parts(source.cast::<u8>(), size).to_vec();
        let _ = GlobalUnlock(global);
        copied
    };
    // SAFETY: The receiver releases the medium exactly once; it carries no release object.
    unsafe { ReleaseStgMedium(&raw mut medium) };
    bytes
}

/// Enter one STA for real OLE calls on this test thread.
struct Apartment;
impl Apartment {
    fn enter() -> Result<Self, Box<dyn Error>> {
        // SAFETY: Balanced by Drop on this thread.
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok()?;
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: Balances this thread's successful initialization.
        unsafe { CoUninitialize() };
    }
}

#[test]
fn descriptor_names_keep_the_output_leaf_and_refuse_paths_and_devices() {
    let name = descriptor_name("ComfyUI_00012_é.png").map(|name| {
        let length = name.iter().position(|unit| *unit == 0).unwrap_or(name.len());
        String::from_utf16_lossy(&name[..length])
    });
    assert_eq!(name.as_deref(), Some("ComfyUI_00012_é.png"));
    for refused in [
        "",
        ".",
        "..",
        "../escape.png",
        "folder\\image.png",
        "C:image.png",
        "image.png.",
        "image.png ",
        "line\nbreak.png",
        "a?.png",
    ] {
        assert!(descriptor_name(refused).is_none(), "{refused:?}");
    }
    assert!(descriptor_name(&"a".repeat(259)).is_some());
    assert!(descriptor_name(&"a".repeat(260)).is_none());
}

#[test]
fn only_the_virtual_file_formats_in_global_memory_are_offered() {
    let global = TYMED_HGLOBAL.0 as u32;
    assert_eq!(FORMATS.offer(&request(0xc101, global, -1)), Ok(Offer::Descriptor));
    assert_eq!(FORMATS.offer(&request(0xc102, global, 0)), Ok(Offer::Contents));
    assert_eq!(FORMATS.offer(&request(0xc102, global | 4, -1)), Ok(Offer::Contents));
    assert_eq!(FORMATS.offer(&request(0xc102, global, 1)), Err(DV_E_LINDEX));
    assert_eq!(FORMATS.offer(&request(0xc102, 4, 0)), Err(DV_E_TYMED));
    assert_eq!(FORMATS.offer(&request(15, global, -1)), Err(DV_E_FORMATETC), "no CF_HDROP");
    let thumbnail = FORMATETC { dwAspect: 2, ..request(0xc101, global, -1) };
    assert_eq!(FORMATS.offer(&thumbnail), Err(DV_E_FORMATETC));
}

#[test]
fn the_source_drops_on_release_and_cancels_on_escape_or_secondary_press() {
    assert_eq!(continuation(false, MK_LBUTTON), HRESULT(0));
    assert_eq!(continuation(false, MODIFIERKEYS_FLAGS(MK_LBUTTON.0 | MK_CONTROL.0)), HRESULT(0));
    assert_eq!(continuation(false, MODIFIERKEYS_FLAGS(0)), DRAGDROP_S_DROP);
    assert_eq!(continuation(true, MK_LBUTTON), DRAGDROP_S_CANCEL);
    assert_eq!(
        continuation(false, MODIFIERKEYS_FLAGS(MK_LBUTTON.0 | MK_RBUTTON.0)),
        DRAGDROP_S_CANCEL
    );
}

#[test]
fn the_data_object_supplies_the_original_name_and_exact_bytes() -> TestResult {
    let _apartment = Apartment::enter()?;
    let released = Arc::new(AtomicU64::new(0));
    let bytes = b"\x89PNG\r\n\x1a\nexact output bytes".to_vec();
    let data = data_object(drag("ComfyUI_00042_.png", bytes.clone(), &released)?)?;
    let formats = Formats::register()?;
    let global = TYMED_HGLOBAL.0 as u32;
    // SAFETY: Real OLE calls on this STA with live local format descriptions.
    unsafe {
        assert_eq!(data.QueryGetData(&request(formats.descriptor, global, -1)), HRESULT(0));
        assert_eq!(data.QueryGetData(&request(15, global, -1)), DV_E_FORMATETC);
        let descriptor = take(data.GetData(&request(formats.descriptor, global, -1))?);
        assert_eq!(descriptor.len(), size_of::<FILEGROUPDESCRIPTORW>());
        let group = descriptor.as_ptr().cast::<FILEGROUPDESCRIPTORW>().read_unaligned();
        // The shell's descriptor structures are packed; copy each field before inspecting it.
        let (items, [file]) = ({ group.cItems }, { group.fgd });
        assert_eq!(items, 1);
        let (name, flags, high, low) =
            ({ file.cFileName }, { file.dwFlags }, { file.nFileSizeHigh }, { file.nFileSizeLow });
        let length = name.iter().position(|unit| *unit == 0).unwrap_or(0);
        assert_eq!(String::from_utf16(&name[..length])?, "ComfyUI_00042_.png");
        assert_eq!(flags & FD_FILESIZE.0 as u32, FD_FILESIZE.0 as u32);
        assert_eq!((u64::from(high) << 32) | u64::from(low), bytes.len() as u64);
        assert_eq!(take(data.GetData(&request(formats.contents, global, 0))?), bytes);
        assert!(data.GetData(&request(formats.contents, global, 1)).is_err());
        let enumerator = data.EnumFormatEtc(DATADIR_GET.0 as u32)?;
        let mut offered = [FORMATETC::default(), FORMATETC::default(), FORMATETC::default()];
        let mut fetched = 0;
        let _ = enumerator.Next(&mut offered, Some(&raw mut fetched));
        assert_eq!(fetched, 2);
        assert_eq!(
            [offered[0].cfFormat, offered[1].cfFormat],
            [formats.descriptor, formats.contents]
        );
    }
    assert_eq!(released.load(Ordering::SeqCst), 0, "the claim lives with the data object");
    drop(data);
    assert_eq!(released.load(Ordering::SeqCst), bytes.len() as u64);
    Ok(())
}

#[test]
fn outputs_without_bytes_or_a_droppable_name_never_reach_ole() -> TestResult {
    let released = Arc::new(AtomicU64::new(0));
    assert!(VirtualFile::new(drag("image.png", Vec::new(), &released)?, FORMATS).is_err());
    assert!(VirtualFile::new(drag("sub/image.png", vec![1], &released)?, FORMATS).is_err());
    Ok(())
}
