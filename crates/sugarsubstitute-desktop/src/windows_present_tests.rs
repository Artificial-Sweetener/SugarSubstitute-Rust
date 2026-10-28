// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise complete-buffer preparation and real GDI exposure on an owned inactive desktop.
use super::*;
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryLease, MemoryRelease, SurfaceId},
    presentation::{FrameIdentity, FrameShape, FrameValidator},
};
use windows::{
    Win32::{
        Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM},
        Graphics::{
            Dwm::{DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BLURBEHIND, DwmEnableBlurBehindWindow},
            Gdi::{CreateRectRgn, DeleteObject, GetCurrentObject, HRGN, OBJ_BITMAP},
        },
        System::{
            Com::CoCreateGuid,
            LibraryLoader::GetModuleHandleW,
            StationsAndDesktops::{
                CloseDesktop, CreateDesktopW, DESKTOP_CONTROL_FLAGS, DESKTOP_CREATEWINDOW,
                DESKTOP_READOBJECTS, DESKTOP_WRITEOBJECTS, GetThreadDesktop, HDESK,
                SetThreadDesktop,
            },
            Threading::GetCurrentThreadId,
        },
        UI::WindowsAndMessaging::{
            CS_HREDRAW, CS_OWNDC, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow,
            RegisterClassExW, SW_HIDE, SW_SHOWNOACTIVATE, ShowWindow, UnregisterClassW,
            WNDCLASSEXW, WS_CLIPSIBLINGS, WS_EX_ACCEPTFILES, WS_EX_APPWINDOW, WS_POPUP, WS_SYSMENU,
        },
    },
    core::PCWSTR,
};

struct Release(Arc<AtomicUsize>);
impl MemoryRelease for Release {
    fn release(&self, _bytes: u64) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
fn frame() -> Result<(PreparedFrame, Arc<AtomicUsize>), Box<dyn Error>> {
    let shape = FrameShape::new(558, 356, 1.0)?;
    let released = Arc::new(AtomicUsize::new(0));
    let lease = MemoryLease::new(2 * shape.byte_len() as u64, Box::new(Release(released.clone())));
    let mut pixels = vec![0; shape.byte_len()];
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[8, 10, 12, 247]);
    }
    let mut validator = FrameValidator::new(
        shape,
        FrameIdentity {
            surface: SurfaceId::new(1)?,
            state_generation: Generation::new(1)?,
            sequence: Generation::new(1)?,
        },
        pixels,
        lease,
    )?;
    while !validator.validate_next(NonZeroUsize::new(4096).ok_or("validation chunk absent")?)? {}
    Ok((validator.finish()?, released))
}

/// Keep every display effect off the input desktop, without creating an execution thread.
struct Fixture {
    previous: HDESK,
    desktop: Option<HDESK>,
    attached: bool,
    instance: HINSTANCE,
    class: Vec<u16>,
    registered: bool,
    window: HWND,
    region: HRGN,
}
impl Fixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        // SAFETY: These scalar calls borrow only this fresh qualification thread's identities.
        let (previous, guid, module) = unsafe {
            (GetThreadDesktop(GetCurrentThreadId()), CoCreateGuid(), GetModuleHandleW(None))
        };
        let previous = previous?;
        let guid = guid?;
        let mut owner = Self {
            previous,
            desktop: None,
            attached: false,
            instance: HINSTANCE(module?.0),
            class: format!("SugarSubstitute-Gdi-{guid:?}").encode_utf16().chain(Some(0)).collect(),
            registered: false,
            window: HWND::default(),
            region: HRGN::default(),
        };
        // SAFETY: The GUID name creates an owned inactive desktop; switch/activation access is absent.
        let desktop = unsafe {
            CreateDesktopW(
                PCWSTR(owner.class.as_ptr()),
                None,
                None,
                DESKTOP_CONTROL_FLAGS(0),
                DESKTOP_CREATEWINDOW.0 | DESKTOP_READOBJECTS.0 | DESKTOP_WRITEOBJECTS.0,
                None,
            )
        }?;
        owner.desktop = Some(desktop);
        // SAFETY: This fresh thread owns no HWNDs before attaching to the newly created desktop.
        unsafe { SetThreadDesktop(owner.desktop.ok_or("private desktop absent")?) }?;
        owner.attached = true;
        let descriptor = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_OWNDC | CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(procedure),
            hInstance: owner.instance,
            lpszClassName: PCWSTR(owner.class.as_ptr()),
            ..Default::default()
        };
        // SAFETY: The descriptor and retained class-name storage outlive this synchronous registration.
        if unsafe { RegisterClassExW(&descriptor) } == 0 {
            return Err(windows::core::Error::from_thread().into());
        }
        owner.registered = true;
        // SAFETY: This original affine window begins hidden and remains on the owned inactive desktop.
        owner.window = unsafe {
            CreateWindowExW(
                WS_EX_APPWINDOW | WS_EX_ACCEPTFILES,
                PCWSTR(owner.class.as_ptr()),
                PCWSTR(owner.class.as_ptr()),
                WS_POPUP | WS_CLIPSIBLINGS | WS_SYSMENU,
                0,
                0,
                558,
                356,
                None,
                None,
                Some(owner.instance),
                None,
            )
        }?;
        // SAFETY: This copied region matches Tao's transparent-window DWM configuration.
        owner.region = unsafe { CreateRectRgn(0, 0, -1, -1) };
        if owner.region.is_invalid() {
            return Err(windows::core::Error::from_thread().into());
        }
        let blur = DWM_BLURBEHIND {
            dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
            fEnable: true.into(),
            hRgnBlur: owner.region,
            ..Default::default()
        };
        // SAFETY: The original owned HWND and region remain retained across synchronous DWM setup.
        unsafe { DwmEnableBlurBehindWindow(owner.window, &blur) }?;
        let _material_supported =
            set_attribute(owner.window, DWMWA_SYSTEMBACKDROP_TYPE, 2, "test_mica")?;
        let margins =
            MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
        // SAFETY: The owned window receives the same full-client glass descriptor as production.
        unsafe { DwmExtendFrameIntoClientArea(owner.window, &margins) }?;
        Ok(owner)
    }
    fn visible(&self) -> bool {
        // SAFETY: This passive query borrows only the original still-owned window.
        unsafe { IsWindowVisible(self.window) }.as_bool()
    }
    fn show(&self, visible: bool) {
        // SAFETY: Only this fixture's inactive-desktop window changes; ShowWindow returns prior state.
        let _previous =
            unsafe { ShowWindow(self.window, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE }) };
        assert_eq!(self.visible(), visible);
    }
    fn close(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.window.is_invalid() {
            // SAFETY: Destroy only this fixture's original creating-thread window.
            unsafe { DestroyWindow(self.window) }?;
            self.window = HWND::default();
        }
        if !self.region.is_invalid() {
            // SAFETY: DWM copied the region; its exact owned original is no longer borrowed.
            unsafe { DeleteObject(self.region.into()) }.ok()?;
            self.region = HRGN::default();
        }
        if self.registered {
            // SAFETY: The only class instance retired and its name/module storage remains retained.
            unsafe { UnregisterClassW(PCWSTR(self.class.as_ptr()), Some(self.instance)) }?;
            self.registered = false;
        }
        if self.attached {
            // SAFETY: All fixture HWNDs retired before restoring the original borrowed thread desktop.
            unsafe { SetThreadDesktop(self.previous) }?;
            self.attached = false;
        }
        if let Some(desktop) = self.desktop {
            // SAFETY: No thread or HWND remains attached to this exact owned desktop handle.
            unsafe { CloseDesktop(desktop) }?;
            self.desktop = None;
        }
        Ok(())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(cause) = self.close() {
            tracing::error!(operation="present_fixture_retirement", error=%cause, "Native fixture custody remains retained");
        }
    }
}
/// # Safety
/// Windows invokes this procedure only for the registered class with its native callback arguments.
unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    word: WPARAM,
    value: LPARAM,
) -> LRESULT {
    // SAFETY: Preserve the original documented window procedure without retaining borrowed arguments.
    unsafe { DefWindowProcW(window, message, word, value) }
}

/// Preparation while covered must succeed; every actual exposure still checks the same live GDI copy.
#[test]
fn hidden_complete_frame_reuses_same_native_buffer_across_checked_show_hide_exposure()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    disable_splash_transitions(fixture.window)?;
    let (frame, released) = frame()?;
    let (replacement, replacement_released) = self::frame()?;
    let mut retained = None;
    let dib = prepare_pixels(&mut retained, &frame, NativeMaterial::Mica, 32)?;
    assert!(!fixture.visible());
    assert!(dib.pixels_mut()?.chunks_exact(4).all(|pixel| pixel == [12, 10, 8, 247]));
    assert!(frame.claim_storage(1).is_err(), "the complete native copy must remain admitted");
    let original_dc = dib.dc();
    // SAFETY: Observe only the live bitmap selected in this original owned source DC.
    let original_bitmap = unsafe { GetCurrentObject(original_dc, OBJ_BITMAP) };
    submit_prepared(fixture.window, dib, None)?;
    for visible in [true, false, true] {
        fixture.show(visible);
        let dib = prepare_pixels(&mut retained, &frame, NativeMaterial::Mica, 32)?;
        assert_eq!(dib.dc(), original_dc);
        // SAFETY: The same affine admitted memory DC remains live across every visibility transition.
        let selected = unsafe { GetCurrentObject(dib.dc(), OBJ_BITMAP) };
        assert_eq!(selected, original_bitmap);
        submit_prepared(fixture.window, dib, None)?;
        if visible {
            copy_to_window(fixture.window, dib, None)?;
        }
        assert!(dib.pixels_mut()?.chunks_exact(4).all(|pixel| pixel == [12, 10, 8, 247]));
    }
    let dib = prepare_pixels(&mut retained, &replacement, NativeMaterial::Mica, 32)?;
    assert_eq!(dib.dc(), original_dc);
    // SAFETY: Accepted replacement pixels reuse the same retained native bitmap and its affine DC.
    assert_eq!(unsafe { GetCurrentObject(dib.dc(), OBJ_BITMAP) }, original_bitmap);
    drop(frame);
    assert_eq!(released.load(Ordering::SeqCst), 1, "old preparation credit remained pinned");
    drop(replacement);
    assert_eq!(replacement_released.load(Ordering::SeqCst), 0);
    dib.close()?;
    let failure = copy_to_window(fixture.window, dib, None)
        .err()
        .ok_or("invalid exposure source was accepted")?;
    assert!(failure.to_string().starts_with("present_cpu_frame:"));
    assert!(failure.source().is_some());
    retained = None;
    fixture.close()?;
    drop((retained, fixture));
    assert_eq!(replacement_released.load(Ordering::SeqCst), 1);
    Ok(())
}

/// Actual DWM accepts the splash policy before showing either process's native surface.
#[test]
fn splash_transitions_are_disabled_before_offscreen_show_and_rejected_for_invalid_windows()
-> Result<(), Box<dyn Error>> {
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
    for _ in 0..2 {
        let mut fixture = Fixture::new()?;
        assert!(!fixture.visible());
        disable_splash_transitions(fixture.window)?;
        let mut original = RECT::default();
        // SAFETY: This fixture owns the hidden window and writable rectangle storage.
        unsafe { GetWindowRect(fixture.window, &mut original) }?;
        for visible in [true, false, true] {
            fixture.show(visible);
            let mut actual = RECT::default();
            // SAFETY: Only the fixture's retained inactive-desktop window is queried.
            unsafe { GetWindowRect(fixture.window, &mut actual) }?;
            assert_eq!(actual, original);
        }
        fixture.close()?;
    }
    let failure =
        disable_splash_transitions(HWND::default()).err().ok_or("Invalid HWND accepted")?;
    assert!(failure.to_string().starts_with("disable_splash_transitions:"));
    assert!(failure.source().is_some());
    Ok(())
}
