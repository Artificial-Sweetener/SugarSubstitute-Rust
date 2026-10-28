// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Capture actual native glyphs and the same page scrollbar from their original retained window.
use super::*;
use crate::pixel_capture::PixelCapture;

#[derive(Default)]
pub(super) struct Evidence {
    pub(super) enabled: bool,
    pub(super) expected_shape: Option<FrameShape>,
    surfaces: usize,
    text: usize,
    scrollbar: usize,
    finished: usize,
}
impl PixelCapture for Evidence {
    fn surface(&mut self, frame: &PreparedFrame) {
        assert_eq!(Some(frame.shape()), self.expected_shape);
        self.surfaces += 1;
    }
    fn overlay(
        &mut self,
        control: Option<u16>,
        bounds: [i32; 4],
        clip: Option<[i32; 4]>,
        stride: usize,
        pixels: &[u8],
    ) {
        let [left, top, right, bottom] = bounds;
        assert!(left >= 0 && top >= 0 && right > left && bottom > top);
        assert!(stride >= (right - left) as usize * 4);
        assert!(pixels.len() >= stride * (bottom - top) as usize);
        match control {
            Some(5) => {
                self.text += 1;
                assert!(clip.is_some_and(|clip| clip[1] > top));
                assert!(
                    pixels
                        .chunks_exact(4)
                        .any(|pixel| pixel[0] > 100 && pixel[1] > 100 && pixel[2] > 100),
                    "Native field glyphs missing from capture"
                );
            }
            None => {
                self.scrollbar += 1;
                for shade in [44, 159] {
                    assert!(
                        pixels.chunks_exact(4).any(|pixel| pixel[..3] == [shade; 3]),
                        "Original page scrollbar lost its baseline track or thumb palette"
                    );
                }
            }
            _ => assert_eq!(control, Some(5), "Unexpected native overlay"),
        }
    }
    fn finished(&mut self) {
        self.finished += 1;
    }
}
#[test]
fn native_installer_capture_contains_original_text_and_scroll_pixels() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(
            module_path!(),
            "::native_installer_capture_contains_original_text_and_scroll_pixels"
        ),
        qualify,
    )
}
fn qualify() -> TestResult {
    let mut fixture = Fixture::new(false)?;
    fixture.capture.enabled = true;
    run(
        super::super::super::Window {
            title: "Native installer loop qualification".into(),
            visible: true,
            close: ControlId::new(1)?,
        },
        &mut fixture,
    )?;
    assert!(fixture.finished);
    assert_eq!(
        (
            fixture.capture.surfaces,
            fixture.capture.text,
            fixture.capture.scrollbar,
            fixture.capture.finished
        ),
        (1, 1, 1, 1)
    );
    Ok(())
}
