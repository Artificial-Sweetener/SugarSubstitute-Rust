// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Parse supported SGR from fixed scratch; unsupported terminal commands never execute.
use sugarsubstitute_contracts::transcript::{AnsiColor, TextStyle};
#[derive(Default)]
enum State {
    #[default]
    Ground,
    Escape,
    Csi,
    DiscardCsi,
    Osc {
        escape: bool,
    },
}
pub(super) struct Ansi {
    state: State,
    scratch: [u8; 64],
    length: usize,
    pub(super) style: TextStyle,
}
impl Default for Ansi {
    fn default() -> Self {
        Self { state: State::Ground, scratch: [0; 64], length: 0, style: TextStyle::default() }
    }
}
impl Ansi {
    /// Return visible byte plus the number of newly detected malformed escapes.
    pub(super) fn push(&mut self, byte: u8) -> (Option<u8>, u64) {
        match self.state {
            State::Ground if byte == 0x1b => {
                self.state = State::Escape;
                (None, 0)
            }
            State::Ground => (Some(byte), 0),
            State::Escape => {
                self.length = 0;
                match byte {
                    b'[' => self.state = State::Csi,
                    b']' => self.state = State::Osc { escape: false },
                    0x1b => return (None, 1),
                    _ => {
                        self.state = State::Ground;
                        return (Some(byte), 1);
                    }
                }
                (None, 0)
            }
            State::Csi => {
                if (0x40..=0x7e).contains(&byte) {
                    self.state = State::Ground;
                    let malformed = byte == b'm' && !self.sgr();
                    (None, u64::from(malformed))
                } else if self.length == self.scratch.len() {
                    self.state = State::DiscardCsi;
                    (None, 1)
                } else if byte.is_ascii_digit() || byte == b';' {
                    self.scratch[self.length] = byte;
                    self.length += 1;
                    (None, 0)
                } else {
                    self.state = State::DiscardCsi;
                    (None, 1)
                }
            }
            State::DiscardCsi => {
                if (0x40..=0x7e).contains(&byte) {
                    self.state = State::Ground;
                }
                (None, 0)
            }
            State::Osc { escape } => {
                if byte == 7 || (escape && byte == b'\\') {
                    self.state = State::Ground;
                } else {
                    self.state = State::Osc { escape: byte == 0x1b };
                    if self.length < 65 {
                        self.length += 1;
                        if self.length == 65 {
                            return (None, 1);
                        }
                    }
                }
                (None, 0)
            }
        }
    }
    pub(super) fn finish(&mut self) -> u64 {
        let incomplete = match self.state {
            State::Ground | State::DiscardCsi => false,
            State::Osc { .. } if self.length == 65 => false,
            _ => true,
        };
        self.state = State::Ground;
        self.length = 0;
        u64::from(incomplete)
    }
    fn sgr(&mut self) -> bool {
        let mut values = [0_u16; 32];
        let mut count = 0;
        let mut value = 0_u16;
        for byte in &self.scratch[..self.length] {
            if *byte == b';' {
                if count == values.len() {
                    return false;
                }
                values[count] = value;
                count += 1;
                value = 0;
            } else {
                let Some(next) =
                    value.checked_mul(10).and_then(|v| v.checked_add((byte - b'0') as u16))
                else {
                    return false;
                };
                value = next;
            }
        }
        if count == values.len() {
            return false;
        }
        values[count] = value;
        count += 1;
        let mut style = self.style;
        let mut cursor = 0;
        while cursor < count {
            let value = values[cursor];
            cursor += 1;
            match value {
                0 => style = TextStyle::default(),
                1 => style.bold = true,
                22 => style.bold = false,
                30..=37 => style.foreground = AnsiColor::Indexed((value - 30) as u8),
                40..=47 => style.background = AnsiColor::Indexed((value - 40) as u8),
                90..=97 => style.foreground = AnsiColor::Indexed((value - 90 + 8) as u8),
                100..=107 => style.background = AnsiColor::Indexed((value - 100 + 8) as u8),
                39 => style.foreground = AnsiColor::Default,
                49 => style.background = AnsiColor::Default,
                38 | 48 => {
                    let Some((color, used)) = extended(&values[cursor..count]) else {
                        return false;
                    };
                    cursor += used;
                    if value == 38 {
                        style.foreground = color;
                    } else {
                        style.background = color;
                    }
                }
                _ => {}
            }
        }
        self.style = style;
        true
    }
}
fn extended(values: &[u16]) -> Option<(AnsiColor, usize)> {
    match values {
        [5, index, ..] if *index <= 255 => Some((AnsiColor::Indexed(*index as u8), 2)),
        [2, r, g, b, ..] if *r <= 255 && *g <= 255 && *b <= 255 => {
            Some((AnsiColor::Rgb(*r as u8, *g as u8, *b as u8), 4))
        }
        _ => None,
    }
}
