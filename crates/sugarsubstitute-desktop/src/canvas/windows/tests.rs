// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify the native canvas loop without showing or activating a window.

use super::*;
use ::windows::Win32::UI::{
    Input::KeyboardAndMouse::{GetActiveWindow, GetCapture, GetFocus},
    WindowsAndMessaging::{IsWindowVisible, SendMessageW, WM_PAINT},
};
use std::{cell::Cell, rc::Rc, sync::Arc};
use sugarsubstitute_canvas::{CursorIntent, DamagePlan};
use sugarsubstitute_contracts::execution::{CompletionWake, Generation, SurfaceId};

fn stamp(generation: u64) -> Result<FrameStamp, Box<dyn Error>> {
    Ok(FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(generation)? })
}

fn thread_ui_state() -> (HWND, HWND, HWND) {
    // SAFETY: These read-only calls return handles owned by the current UI thread.
    unsafe { (GetActiveWindow(), GetFocus(), GetCapture()) }
}

#[test]
fn cpu_upload_acknowledgement_is_pending_only_and_precedes_display_state()
-> Result<(), Box<dyn Error>> {
    let old = stamp(1)?;
    let next = stamp(2)?;
    let mut displayed = Some(old);
    let mut acknowledgements = Vec::new();
    assert!(record_cpu_upload(&mut displayed, next, true, |target| {
        acknowledgements.push(target);
        Ok::<(), Box<dyn Error>>(())
    })?);
    assert_eq!(acknowledgements, [next]);
    assert_eq!(displayed, Some(next));

    assert!(!record_cpu_upload(&mut displayed, old, false, |_| {
        Err::<(), Box<dyn Error>>("Acknowledged an already-presented exposure".into())
    })?);
    assert_eq!(acknowledgements, [next]);
    assert_eq!(displayed, Some(old));

    let failed = record_cpu_upload(&mut displayed, next, true, |_| Err("acknowledgement"));
    assert!(failed.is_err());
    assert_eq!(displayed, Some(old));
    Ok(())
}

struct StartupFixture {
    wake: Option<Arc<dyn CompletionWake>>,
    expected_extent: Extent,
    expected_selection: PathBuf,
    window: Rc<Cell<HWND>>,
    original_ui_state: (HWND, HWND, HWND),
    target: FrameStamp,
    damage: Option<DamagePlan>,
    pixels: Vec<u32>,
    attached: bool,
    resized: bool,
    picker_pending: bool,
    picker_taken: bool,
    selected: bool,
    pending: bool,
    acknowledgements: Vec<FrameStamp>,
    closing: bool,
    close_calls: usize,
    finished: bool,
    selecting: Rc<Cell<bool>>,
    nested_exposure: Rc<Cell<bool>>,
}

impl StartupFixture {
    fn new(
        expected_extent: Extent,
        expected_selection: PathBuf,
        window: Rc<Cell<HWND>>,
        original_ui_state: (HWND, HWND, HWND),
    ) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            wake: None,
            expected_extent,
            expected_selection,
            window,
            original_ui_state,
            target: stamp(1)?,
            damage: None,
            pixels: Vec::new(),
            attached: false,
            resized: false,
            picker_pending: true,
            picker_taken: false,
            selected: false,
            pending: false,
            acknowledgements: Vec::new(),
            closing: false,
            close_calls: 0,
            finished: false,
            selecting: Rc::new(Cell::new(false)),
            nested_exposure: Rc::new(Cell::new(false)),
        })
    }

    fn wake(&self) -> Result<(), Box<dyn Error>> {
        self.wake.as_ref().ok_or("Completion wake was not attached")?.wake()?;
        Ok(())
    }
}

impl Lifetime for StartupFixture {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        if self.wake.replace(wake).is_some() {
            return Err("Duplicate startup".into());
        }
        Ok(())
    }

    fn attach_window(&mut self, _target: WindowTarget) -> Result<(), Box<dyn Error>> {
        if self.wake.is_none() || std::mem::replace(&mut self.attached, true) {
            return Err("Window attached out of order or more than once".into());
        }
        Ok(())
    }

    fn resize(&mut self, extent: Extent) -> Result<(), Box<dyn Error>> {
        if !self.attached || extent != self.expected_extent {
            return Err("Unexpected hidden canvas extent".into());
        }
        self.resized = true;
        Ok(())
    }

    fn input(&mut self, input: Input) -> Result<(), Box<dyn Error>> {
        match input {
            Input::DisplayRefresh(_) | Input::Unfocused => Ok(()),
            _ => Err("Hidden startup received user input".into()),
        }
    }

    fn feedback(&self) -> Feedback {
        Feedback { cursor: CursorIntent::Arrow, capture: false }
    }

    fn take_picker_request(&mut self) -> bool {
        let requested = std::mem::take(&mut self.picker_pending);
        self.picker_taken |= requested;
        requested
    }

    fn selected(
        &mut self,
        selection: Result<Option<PathBuf>, Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>> {
        let selected = selection?.ok_or("Picker unexpectedly cancelled")?;
        if !self.picker_taken || selected != self.expected_selection || self.selected {
            return Err("Unexpected picker result or ordering".into());
        }
        self.damage = Some(DamagePlan::full(self.expected_extent));
        self.pixels = vec![0xff_12_34_56; usize::try_from(self.expected_extent.pixels())?];
        self.selected = true;
        self.pending = true;
        self.wake()
    }

    fn poll(&mut self, _elapsed: Duration) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn frame(&self) -> Option<CpuFrame<'_>> {
        if !self.selected {
            return None;
        }
        Some(CpuFrame {
            kind: CpuFrameKind::Prepared,
            pending: self.pending,
            target: self.target,
            base: None,
            damage: self.damage.as_ref()?,
            pixels: &self.pixels,
        })
    }

    fn did_present_cpu(&mut self, target: FrameStamp) -> Result<(), Box<dyn Error>> {
        if !self.pending || target != self.target || !self.acknowledgements.is_empty() {
            return Err("Foreign or duplicate CPU presentation acknowledgement".into());
        }
        let window = self.window.get();
        if window.is_invalid() {
            return Err("CPU presentation has no native peer".into());
        }
        // SAFETY: The presentation callback retains this live native window on its UI thread.
        let visible = unsafe { IsWindowVisible(window) }.as_bool();
        if visible || thread_ui_state() != self.original_ui_state {
            return Err("CPU presentation changed hidden-window input or activation state".into());
        }
        self.pending = false;
        self.acknowledgements.push(target);
        self.closing = true;
        self.wake()
    }

    fn take_gpu_presentation(&mut self) -> Option<GpuPresentation> {
        None
    }

    fn did_present_gpu(&mut self, _target: FrameStamp) -> Result<(), Box<dyn Error>> {
        Err("Hidden CPU fixture received a GPU acknowledgement".into())
    }

    fn expose(&mut self) {
        if self.selecting.get() {
            self.nested_exposure.set(true);
        }
    }

    fn title(&self) -> &str {
        "Hidden canvas qualification"
    }

    fn next_deadline(&self) -> Option<Instant> {
        None
    }

    fn is_closing(&self) -> bool {
        self.closing
    }

    fn close(&mut self) -> Result<(), Box<dyn Error>> {
        self.close_calls += 1;
        Ok(())
    }

    fn settled(&mut self) -> Result<bool, Box<dyn Error>> {
        Ok(self.close_calls == 1)
    }

    fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.attached
            || !self.resized
            || !self.picker_taken
            || !self.selected
            || self.acknowledgements != [self.target]
            || self.close_calls != 1
            || self.pending
            || std::mem::replace(&mut self.finished, true)
        {
            return Err("Hidden startup did not complete exactly once".into());
        }
        Ok(())
    }
}

#[test]
fn hidden_empty_startup_selects_and_presents_one_cpu_frame_without_activation()
-> Result<(), Box<dyn Error>> {
    let extent = Extent::new(96, 64)?;
    let selected = PathBuf::from("fixture-selected-image.png");
    let original = thread_ui_state();
    let native_window = Rc::new(Cell::new(HWND::default()));
    let mut fixture =
        StartupFixture::new(extent, selected.clone(), native_window.clone(), original)?;
    let picker_calls = Rc::new(Cell::new(0));
    let observed_calls = picker_calls.clone();
    let selecting = fixture.selecting.clone();
    let nested_exposure = fixture.nested_exposure.clone();

    run_with_picker(
        WindowOptions { title: "Hidden canvas qualification".to_owned(), extent, visible: false },
        &mut fixture,
        move |builder| {
            crate::file_dialog::Picker::with_selection(
                builder,
                Box::new(move |hwnd, kind, _| {
                    assert_eq!(kind, SelectionKind::File);
                    picker_calls.set(picker_calls.get() + 1);
                    native_window.set(hwnd);
                    // SAFETY: Tao retains this valid peer throughout the synchronous picker boundary.
                    if unsafe { IsWindowVisible(hwnd) }.as_bool() || thread_ui_state() != original {
                        return Err(
                            "Hidden picker boundary changed input or activation state".into()
                        );
                    }
                    selecting.set(true);
                    // SAFETY: Repaint only this invisible fixture. The real Tao callback must run while
                    // the external shell boundary is active, without borrowing its handler recursively.
                    unsafe {
                        SendMessageW(hwnd, WM_PAINT, None, None);
                    }
                    selecting.set(false);
                    if !nested_exposure.get() {
                        return Err(
                            "Nested native events were buffered behind the application callback"
                                .into(),
                        );
                    }
                    Ok(Some(selected.clone()))
                }),
            )
        },
    )?;

    assert_eq!(observed_calls.get(), 1);
    assert!(fixture.nested_exposure.get());
    assert!(fixture.finished);
    assert_eq!(thread_ui_state(), original);
    Ok(())
}
