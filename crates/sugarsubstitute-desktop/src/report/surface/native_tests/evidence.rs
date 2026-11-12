// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep what the report window presented: its prepared frame composited with the native overlays,
//! taken through the host's pixel capture after presentation, never from the screen.
use super::*;
use sugarsubstitute_contracts::presentation::PreparedFrame;

/// One kept capture: name, width, height and opaque BGRA pixels.
pub(super) type Capture = (&'static str, u32, u32, Vec<u8>);

/// Captures requested by the script and taken by the host after its next presentation, and
/// whether the report had nothing left to prepare after the last turn.
#[derive(Default)]
pub(super) struct Captures {
    pub(super) request: Option<&'static str>,
    pub(super) taken: Vec<Capture>,
    pub(super) idle: bool,
}

/// Composite the presented frame and its native overlays, then flatten over the fallback
/// material color the window shows where its pixels are transparent.
pub(super) struct Observer {
    pub(super) shared: Rc<RefCell<Captures>>,
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) pixels: Vec<u8>,
}

impl crate::pixel_capture::PixelCapture for Observer {
    fn surface(&mut self, frame: &PreparedFrame) {
        self.width = frame.shape().width() as usize;
        self.height = frame.shape().height() as usize;
        self.pixels = frame.pixels().to_vec();
    }
    fn overlay(
        &mut self,
        _: Option<u16>,
        bounds: [i32; 4],
        clip: Option<[i32; 4]>,
        stride: usize,
        pixels: &[u8],
    ) {
        let [left, top, right, bottom] = clip.unwrap_or(bounds);
        for y in top.max(bounds[1]).max(0)..bottom.min(bounds[3]).min(self.height as i32) {
            for x in left.max(bounds[0]).max(0)..right.min(bounds[2]).min(self.width as i32) {
                let source = (y - bounds[1]) as usize * stride + (x - bounds[0]) as usize * 4;
                let Some(bgra) = pixels.get(source..source + 4) else { continue };
                let target = (y as usize * self.width + x as usize) * 4;
                let inverse = 255 - u32::from(bgra[3]);
                for (channel, value) in [bgra[2], bgra[1], bgra[0], bgra[3]].into_iter().enumerate()
                {
                    let below = u32::from(self.pixels[target + channel]);
                    self.pixels[target + channel] =
                        (u32::from(value) + below * inverse / 255) as u8;
                }
            }
        }
    }
    fn finished(&mut self) {
        let mut shared = self.shared.borrow_mut();
        let Some(name) = shared.request.take() else { return };
        let corner = (self.height.saturating_sub(2) * self.width + 2) * 4;
        let background: u32 =
            if self.pixels.get(corner).is_some_and(|v| *v < 128) { 32 } else { 243 };
        let mut bgra = Vec::with_capacity(self.pixels.len());
        for rgba in self.pixels.chunks_exact(4) {
            let flat = |value: u8| {
                (u32::from(value) + background * (255 - u32::from(rgba[3])) / 255) as u8
            };
            bgra.extend_from_slice(&[flat(rgba[2]), flat(rgba[1]), flat(rgba[0]), 255]);
        }
        shared.taken.push((name, self.width as u32, self.height as u32, bgra));
    }
}

/// Keep captures as 32-bit bitmaps when a proof directory is configured.
pub(super) fn keep(captures: &[Capture]) -> TestResult {
    let Some(directory) = std::env::var_os("SUGARSUBSTITUTE_REPORT_PROOF_DIR") else {
        return Ok(());
    };
    for (name, width, height, pixels) in captures {
        let mut file = Vec::with_capacity(54 + pixels.len());
        let size = 54 + pixels.len() as u32;
        file.extend_from_slice(b"BM");
        file.extend_from_slice(&size.to_le_bytes());
        file.extend_from_slice(&[0; 4]);
        file.extend_from_slice(&54u32.to_le_bytes());
        file.extend_from_slice(&40u32.to_le_bytes());
        file.extend_from_slice(&(*width as i32).to_le_bytes());
        file.extend_from_slice(&(-(*height as i32)).to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&32u16.to_le_bytes());
        file.extend_from_slice(&[0; 24]);
        file.extend_from_slice(pixels);
        std::fs::write(std::path::Path::new(&directory).join(format!("{name}.bmp")), file)?;
    }
    Ok(())
}

/// A painted capture has the report's ink: dark and light pixels, not one flat color.
pub(super) fn assert_painted(captures: &[Capture]) {
    for (name, width, height, pixels) in captures {
        assert!(*width >= 700 && *height >= 200, "{name}: {width}x{height}");
        let bright =
            pixels.chunks_exact(4).filter(|p| p[0] > 200 && p[1] > 200 && p[2] > 200).count();
        let dark = pixels.chunks_exact(4).filter(|p| p[0] < 60 && p[1] < 60 && p[2] < 60).count();
        assert!(bright > 500 && dark > 500, "{name}: {bright} bright, {dark} dark");
    }
}
