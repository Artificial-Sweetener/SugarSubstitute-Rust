// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Share terminal color interpretation between raster and native text presentation.
use crate::transcript::AnsiColor;
/// Preserve the original terminal's basic palette; extended colors follow ANSI indices.
pub fn console_color(
    value: AnsiColor,
    theme: crate::configuration::Theme,
    background: bool,
) -> Option<[u8; 3]> {
    const BASIC: [[u8; 3]; 16] = [
        [0x17, 0x19, 0x1c],
        [0xf6, 0x61, 0x51],
        [0x33, 0xd1, 0x7a],
        [0xe5, 0xa5, 0x0a],
        [0x35, 0x84, 0xe4],
        [0xc0, 0x61, 0xcb],
        [0x33, 0xc7, 0xde],
        [0xde, 0xdd, 0xda],
        [0x77, 0x76, 0x7b],
        [0xff, 0x7b, 0x72],
        [0x57, 0xe3, 0x89],
        [0xf8, 0xe4, 0x5c],
        [0x62, 0xa0, 0xea],
        [0xdc, 0x8a, 0xdd],
        [0x5b, 0xc8, 0xaf],
        [0xff, 0xff, 0xff],
    ];
    Some(match value {
        AnsiColor::Default if background => return None,
        AnsiColor::Default => {
            if theme == crate::configuration::Theme::Dark {
                [255; 3]
            } else {
                [0; 3]
            }
        }
        AnsiColor::Rgb(r, g, b) => [r, g, b],
        AnsiColor::Indexed(i @ 0..=15) => BASIC[usize::from(i)],
        AnsiColor::Indexed(i @ 16..=231) => {
            let n = i - 16;
            let component = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            [component(n / 36), component((n / 6) % 6), component(n % 6)]
        }
        AnsiColor::Indexed(i) => [8 + 10 * (i - 232); 3],
    })
}

/// Separate admitted allowance for a native document, exact readback, copied style/index
/// storage and affine callback metadata. Opaque OS implementation overhead follows
/// the existing native font/control accounting boundary.
pub const NATIVE_CONSOLE_STORAGE_BYTES: u64 = 2 * 1024 * 1024;
/// Assign one text presentation owner independently of whether output has arrived.
/// This is process-only preparation input, never an IPC permission or persisted format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsolePresentation {
    /// The portable raster owner paints styled text into prepared pixels.
    Raster,
    /// The Windows affine read-only peer paints text; the frame still paints its panel.
    Native,
}
impl ConsolePresentation {
    /// Admit independent native copied storage alongside output and its presenter copy.
    pub const fn storage_bytes(self) -> u64 {
        match self {
            Self::Raster => 0,
            Self::Native => NATIVE_CONSOLE_STORAGE_BYTES,
        }
    }
}
