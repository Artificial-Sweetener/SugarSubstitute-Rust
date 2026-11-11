// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the real native installer loop and clipped peers entirely on a private desktop.
use super::*;
use ::windows::{
    Win32::{
        Foundation::{HWND, RECT},
        Graphics::Gdi::{CreateRectRgn, DeleteObject, ERROR, GetRgnBox, GetWindowRgn, NULLREGION},
        UI::WindowsAndMessaging::{FindWindowW, GetDlgItem, IsWindow},
    },
    core::w,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryLease, MemoryRelease, SurfaceId},
    presentation::{FrameIdentity, FrameValidator},
};
use sugarsubstitute_ui::{availability::Availability, form::Role};

type TestResult = Result<(), Box<dyn Error>>;
struct Release(Arc<AtomicUsize>);
impl MemoryRelease for Release {
    fn release(&self, _: u64) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
struct Fixture {
    inventory: Vec<Control>,
    fields: Vec<Control>,
    frame: Option<PreparedFrame>,
    released: Arc<AtomicUsize>,
    shape: Option<FrameShape>,
    count: usize,
    allocated: usize,
    original: Option<(HWND, HWND)>,
    closing: bool,
    finished: bool,
    fail_presentation: bool,
    scroll: f32,
    exercise_scroll: bool,
    capture: capture::Evidence,
}
impl Fixture {
    fn new(fail_presentation: bool) -> Result<Self, Box<dyn Error>> {
        let field = Control {
            id: ControlId::new(5)?,
            label: "Installation folder",
            bounds: Bounds { x: 20.0, y: -10.0, width: 400.0, height: 33.0 },
            availability: Availability { visible: true, enabled: true },
            role: Role::TextField(r"E:\初期🙂".into()),
            text_revision: 0,
        };
        let mut original = field.clone();
        original.bounds.y = 20.0;
        original.availability.visible = false;
        Ok(Self {
            inventory: vec![original],
            fields: vec![field],
            frame: None,
            released: Arc::new(AtomicUsize::new(0)),
            shape: None,
            count: 0,
            allocated: 0,
            original: None,
            closing: false,
            finished: false,
            fail_presentation,
            scroll: 10.0,
            exercise_scroll: false,
            capture: capture::Evidence::default(),
        })
    }
    fn prepare(&mut self, shape: FrameShape) -> TestResult {
        let lease = MemoryLease::new(
            shape.byte_len() as u64 + storage_bytes(shape, 1)?,
            Box::new(Release(self.released.clone())),
        );
        let mut pixels = vec![0; shape.byte_len()];
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[29, 29, 29, 255]);
        }
        let mut validator = FrameValidator::new(
            shape,
            FrameIdentity {
                surface: SurfaceId::new(2)?,
                state_generation: Generation::new(self.allocated as u64 + 1)?,
                sequence: Generation::new(self.allocated as u64 + 1)?,
            },
            pixels,
            lease,
        )?;
        while !validator.validate_next(std::num::NonZeroUsize::MIN.saturating_add(65535))? {}
        self.frame = Some(validator.finish()?);
        self.allocated += 1;
        self.shape = Some(shape);
        self.capture.expected_shape = Some(shape);
        Ok(())
    }
}
impl Lifetime for Fixture {
    fn picker(&mut self) -> Option<Picker> {
        None
    }
    fn console(&self) -> Option<crate::installer::InstallerConsole<'_>> {
        None
    }
    fn inventory(&self) -> &[Control] {
        &self.inventory
    }
    fn start(&mut self, _: Arc<dyn CompletionWake>) -> TestResult {
        Ok(())
    }
    fn resize(&mut self, shape: FrameShape) -> TestResult {
        self.prepare(shape)
    }
    fn input(&mut self, event: super::super::Event) -> TestResult {
        if let super::super::Event::Scroll(position) = &event {
            assert!(self.exercise_scroll);
            self.scroll = *position;
            self.prepare(self.shape.ok_or("Scroll fixture shape absent")?)?;
        }
        if let super::super::Event::Control(id, Input::Text { value, revision }) = event {
            assert_eq!(id, self.fields[0].id);
            self.fields[0].text_revision = revision;
            if let Some(value) = value {
                self.fields[0].role = Role::TextField(value);
            }
        }
        Ok(())
    }
    fn advance(&mut self, _: Duration) -> TestResult {
        Ok(())
    }
    fn page(&self) -> Option<Page<'_>> {
        self.frame.as_ref().map(|frame| Page {
            frame,
            shell: &[],
            controls: &self.fields,
            drag_bounds: Bounds { x: 0.0, y: 0.0, width: 1180.0, height: 100.0 },
            viewport: Bounds { x: 100.0, y: 120.0, width: 980.0, height: 400.0 },
            scroll: self.scroll,
            content_height: 500.0,
        })
    }
    fn presented(&mut self) -> TestResult {
        // SAFETY: This unique window exists only in the current qualification child's private desktop.
        let hwnd = unsafe { FindWindowW(None, w!("Native installer loop qualification")) }?;
        // SAFETY: Read the exact shared field belonging to the original retained installer window.
        let field = unsafe { GetDlgItem(Some(hwnd), 5) }?;
        if let Some(original) = self.original {
            assert_eq!((hwnd, field), original);
        }
        self.original = Some((hwnd, field));
        // SAFETY: The temporary region is owned locally; querying a window region only copies it.
        let region = unsafe { CreateRectRgn(0, 0, 0, 0) };
        if region.is_invalid() {
            return Err("Qualification region allocation failed".into());
        }
        // SAFETY: Copies the original field region into this locally owned destination.
        let kind = unsafe { GetWindowRgn(field, region) };
        let mut clip = RECT::default();
        // SAFETY: The owned region and output rectangle remain live for this scalar query.
        let observed = unsafe { GetRgnBox(region, &mut clip) };
        // SAFETY: Release only the temporary queried copy, never the window-owned region.
        let closed = unsafe { DeleteObject(region.into()) }.ok();
        closed?;
        assert_ne!(kind.0, ERROR);
        assert_ne!(observed, NULLREGION);
        assert!(clip.top > 0, "Partially scrolled field must obey the page clip");
        assert!(
            self.fields[0].text_revision > 0,
            "Original document revision must be acknowledged"
        );
        self.count += 1;
        if self.fail_presentation {
            return Err("Injected presentation acknowledgement failure".into());
        }
        if self.exercise_scroll {
            self.scroll_gesture(hwnd)?;
        } else if self.count == 1 {
            self.fields[0].role = Role::TextField(r"E:\Selected folder".into());
            self.prepare(self.shape.ok_or("Qualification shape absent")?)?;
        }
        Ok(())
    }
    fn pixel_capture(&mut self) -> Option<&mut dyn crate::pixel_capture::PixelCapture> {
        (self.capture.enabled && self.count == 2).then_some(&mut self.capture)
    }
    fn close_requested(&self) -> bool {
        self.count == if self.exercise_scroll { 3 } else { 2 }
    }
    fn close(&mut self) -> TestResult {
        self.closing = true;
        Ok(())
    }
    fn settled(&mut self) -> Result<bool, Box<dyn Error>> {
        Ok(self.closing)
    }
    fn finish(&mut self) -> TestResult {
        assert!(self.closing);
        if let Some((window, field)) = self.original {
            // SAFETY: Passive validity queries prove original window/peer retirement after loop return.
            assert!(!unsafe { IsWindow(Some(window)) }.as_bool());
            assert_ne!(
                // SAFETY: Only observe this qualification thread's capture after original retirement.
                unsafe { ::windows::Win32::UI::Input::KeyboardAndMouse::GetCapture() },
                window
            );
            // SAFETY: Passive query of the original qualification field, without dereferencing it.
            assert!(!unsafe { IsWindow(Some(field)) }.as_bool());
        }
        // SAFETY: Search only this private-desktop fixture's unique window title after teardown.
        assert!(unsafe { FindWindowW(None, w!("Native installer loop qualification")) }.is_err());
        self.frame = None;
        self.finished = true;
        assert_eq!(self.released.load(Ordering::SeqCst), self.allocated);
        Ok(())
    }
}
fn qualify() -> TestResult {
    let mut fixture = Fixture::new(false)?;
    run(
        super::super::Window {
            title: "Native installer loop qualification".into(),
            visible: true,
            close: ControlId::new(1)?,
        },
        &mut fixture,
    )?;
    assert!(fixture.finished);
    assert_eq!(fixture.count, 2);
    assert_eq!(fixture.fields[0].role, Role::TextField(r"E:\Selected folder".into()));
    Ok(())
}
fn qualify_failure() -> TestResult {
    let mut fixture = Fixture::new(true)?;
    let result = run(
        super::super::Window {
            title: "Native installer loop qualification".into(),
            visible: true,
            close: ControlId::new(1)?,
        },
        &mut fixture,
    );
    assert!(fixture.finished);
    assert!(
        result
            .err()
            .ok_or("Injected failure was accepted")?
            .to_string()
            .contains("Injected presentation")
    );
    Ok(())
}
#[test]
fn original_native_installer_window_presents_edits_clips_and_retires() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(
            module_path!(),
            "::original_native_installer_window_presents_edits_clips_and_retires"
        ),
        qualify,
    )
}
#[test]
fn native_installer_failure_still_retires_original_window_and_field() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(
            module_path!(),
            "::native_installer_failure_still_retires_original_window_and_field"
        ),
        qualify_failure,
    )
}

/// Pointer eligibility and original peer roles use the same clipped page inventory as native input.
#[test]
fn page_projection_rejects_duplicate_roles_and_clips_pointer_hits() -> TestResult {
    let mut fixture = Fixture::new(false)?;
    fixture.inventory[0].role = Role::Button;
    fixture.fields[0].role = Role::Button;
    fixture.prepare(FrameShape::new(1180, 760, 1.0)?)?;
    let page = fixture.page().ok_or("Qualification page absent")?;
    assert!(page.draggable(100.0, 99.0));
    assert!(!page.draggable(100.0, 100.0));
    let projection = Projection::new(&fixture.inventory, &page, ControlId::new(1)?)?;
    assert_eq!(projection.hit(125.0, 115.0, 1.0), None);
    assert_eq!(projection.hit(125.0, 125.0, 1.0), Some(fixture.fields[0].id));
    let mut fields = fixture.fields.clone();
    fields.push(fields[0].clone());
    assert!(
        Projection::new(
            &fixture.inventory,
            &Page { controls: &fields, ..page },
            ControlId::new(1)?
        )
        .is_err()
    );
    fields.pop();
    fields[0].role = Role::TextField(String::new());
    assert!(
        Projection::new(
            &fixture.inventory,
            &Page { controls: &fields, ..page },
            ControlId::new(1)?
        )
        .is_err()
    );
    assert!(storage_bytes(fixture.shape.ok_or("Qualification shape absent")?, 65).is_err());
    Ok(())
}

/// Invalid native page metadata fails before first reveal and still tears down the hidden original.
#[test]
fn native_installer_invalid_page_retires_before_first_reveal() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(module_path!(), "::native_installer_invalid_page_retires_before_first_reveal"),
        qualify_invalid,
    )
}
fn qualify_invalid() -> TestResult {
    let mut fixture = Fixture::new(false)?;
    fixture.fields[0].id = ControlId::new(99)?;
    let result = run(
        super::super::Window {
            title: "Native installer loop qualification".into(),
            visible: true,
            close: ControlId::new(1)?,
        },
        &mut fixture,
    );
    assert!(fixture.finished);
    assert_eq!(fixture.count, 0);
    assert!(
        result
            .err()
            .ok_or("Invalid page was accepted")?
            .to_string()
            .contains("original peer inventory")
    );
    Ok(())
}

#[path = "scroll_tests.rs"]
mod scrolling;

#[path = "capture_tests.rs"]
mod capture;

/// The resolved Tao mode reaches the same shared preparation mode without a forced default.
#[test]
fn resolved_native_theme_keeps_both_preparation_modes() {
    assert_eq!(paint_theme(Theme::Light), sugarsubstitute_contracts::configuration::Theme::Light);
    assert_eq!(paint_theme(Theme::Dark), sugarsubstitute_contracts::configuration::Theme::Dark);
}
