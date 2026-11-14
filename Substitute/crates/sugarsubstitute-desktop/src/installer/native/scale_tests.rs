// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The installer window keeps its logical client size at every display scale: from its first
//! frame, and after each move to a monitor with another scale, whatever size the move left it.
use super::*;
use ::windows::{
    Win32::{
        Foundation::{LPARAM, RECT, WPARAM},
        UI::WindowsAndMessaging::{
            FindWindowW, GetClientRect, GetWindowRect, SendMessageW, WM_DPICHANGED,
        },
    },
    core::w,
};
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryLease, MemoryRelease, SurfaceId},
    presentation::{FrameIdentity, FrameValidator},
};

type TestResult = Result<(), Box<dyn Error>>;

/// The installer's logical client size.
const LOGICAL: [f64; 2] = [1180.0, 760.0];

struct NoRelease;
impl MemoryRelease for NoRelease {
    fn release(&self, _: u64) {}
}

/// Presents one plain frame per size and moves the window through 150%, 175% and 100%.
struct Scales {
    frame: Option<PreparedFrame>,
    shape: Option<FrameShape>,
    frames: u64,
    /// Scales still to visit, as DPI, in order.
    pending: Vec<u32>,
    /// The scale announced last; the next presented frame must be at it.
    announced: Option<u32>,
    /// The suggested window rectangle the announcement points to.
    suggested: Box<RECT>,
    /// Every presented frame's scale and the client size the window really had.
    observed: Vec<(f64, [i32; 2])>,
    done: bool,
    closing: bool,
}

impl Scales {
    fn new() -> Self {
        Self {
            frame: None,
            shape: None,
            frames: 0,
            pending: vec![144, 168, 96],
            announced: None,
            suggested: Box::new(RECT::default()),
            observed: Vec::new(),
            done: false,
            closing: false,
        }
    }

    fn prepare(&mut self, shape: FrameShape) -> TestResult {
        let lease = MemoryLease::new(
            shape.byte_len() as u64 + storage_bytes(shape, 0)?,
            Box::new(NoRelease),
        );
        let pixels = [29, 29, 29, 255].repeat(shape.byte_len() / 4);
        self.frames += 1;
        let mut validator = FrameValidator::new(
            shape,
            FrameIdentity {
                surface: SurfaceId::new(3)?,
                state_generation: Generation::new(self.frames)?,
                sequence: Generation::new(self.frames)?,
            },
            pixels,
            lease,
        )?;
        while !validator.validate_next(std::num::NonZeroUsize::MIN.saturating_add(65535))? {}
        self.frame = Some(validator.finish()?);
        self.shape = Some(shape);
        Ok(())
    }

    /// Announce the next scale the way Windows announces a move to another monitor, with a
    /// suggested rectangle that keeps the window's current size, as observed on a desktop whose
    /// window and monitor scales disagree.
    fn announce(&mut self, window: ::windows::Win32::Foundation::HWND) -> TestResult {
        let Some(dpi) = self.pending.first().copied() else {
            self.done = true;
            return Ok(());
        };
        self.pending.remove(0);
        // SAFETY: The window belongs to this thread; the output rectangle is owned here.
        unsafe { GetWindowRect(window, &raw mut *self.suggested) }?;
        let both = (dpi | (dpi << 16)) as usize;
        let suggested = LPARAM(&raw const *self.suggested as isize);
        // SAFETY: The window belongs to this thread; Windows delivers the message synchronously while
        // the rectangle it points to is borrowed from this fixture.
        unsafe { SendMessageW(window, WM_DPICHANGED, Some(WPARAM(both)), Some(suggested)) };
        self.announced = Some(dpi);
        Ok(())
    }
}

impl Lifetime for Scales {
    fn inventory(&self) -> &[Control] {
        &[]
    }
    fn picker(&mut self) -> Option<Picker> {
        None
    }
    fn console(&self) -> Option<crate::installer::InstallerConsole<'_>> {
        None
    }
    fn start(&mut self, _: Arc<dyn CompletionWake>) -> TestResult {
        Ok(())
    }
    fn resize(&mut self, shape: FrameShape) -> TestResult {
        if self.shape != Some(shape) {
            self.prepare(shape)?;
        }
        Ok(())
    }
    fn input(&mut self, _: super::super::Event) -> TestResult {
        Ok(())
    }
    fn advance(&mut self, _: Duration) -> TestResult {
        Ok(())
    }
    fn page(&self) -> Option<Page<'_>> {
        self.frame.as_ref().map(|frame| Page {
            frame,
            shell: &[],
            controls: &[],
            drag_bounds: Bounds { x: 0.0, y: 0.0, width: 200.0, height: 30.0 },
            viewport: Bounds { x: 10.0, y: 40.0, width: 200.0, height: 100.0 },
            scroll: 0.0,
            content_height: 100.0,
        })
    }
    fn presented(&mut self) -> TestResult {
        // SAFETY: This unique window exists only in the current qualification child's desktop.
        let window = unsafe { FindWindowW(None, w!("Native installer scale qualification")) }?;
        let mut client = RECT::default();
        // SAFETY: The window belongs to this thread; the output rectangle is owned here.
        unsafe { GetClientRect(window, &raw mut client) }?;
        let scale = self.shape.ok_or("Presented frame shape absent")?.scale();
        let size = [client.right - client.left, client.bottom - client.top];
        self.observed.push((scale, size));
        if let Some(dpi) = self.announced {
            let expected = f64::from(dpi) / 96.0;
            if (scale - expected).abs() > 1e-9 {
                return Err(
                    format!("frame at scale {scale} after {dpi} DPI: {:?}", self.observed).into()
                );
            }
        }
        let wanted = LOGICAL.map(|extent| (extent * scale).round() as i32);
        if size != wanted {
            return Err(format!(
                "client {size:?} at scale {scale}, wanted {wanted:?}; presented {:?}",
                self.observed
            )
            .into());
        }
        self.announce(window)
    }
    fn close_requested(&self) -> bool {
        self.done
    }
    fn close(&mut self) -> TestResult {
        self.closing = true;
        Ok(())
    }
    fn settled(&mut self) -> Result<bool, Box<dyn Error>> {
        Ok(self.closing)
    }
    fn finish(&mut self) -> TestResult {
        self.frame = None;
        Ok(())
    }
}

fn qualify() -> TestResult {
    let mut scales = Scales::new();
    run(
        super::super::Window::installer(
            "Native installer scale qualification".into(),
            true,
            ControlId::new(1)?,
        ),
        &mut scales,
    )?;
    let visited: Vec<_> = scales.observed.iter().map(|(scale, _)| *scale).collect();
    assert!(scales.done, "every scale was visited: {:?}", scales.observed);
    assert_eq!(&visited[visited.len() - 3..], &[1.5, 1.75, 1.0]);
    Ok(())
}

/// Created on whatever scale the desktop has, then moved to 150%, 175% and back to 100%, the
/// window's client is 1180 by 760 logical pixels in the frame presented at each scale.
#[test]
fn the_installer_keeps_its_logical_size_at_every_display_scale() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(module_path!(), "::the_installer_keeps_its_logical_size_at_every_display_scale"),
        qualify,
    )
}
