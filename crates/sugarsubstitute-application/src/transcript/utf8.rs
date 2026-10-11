// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode maximal invalid UTF-8 subparts with fixed four-byte/four-character scratch.
#[derive(Default)]
pub(super) struct Decoder {
    bytes: [u8; 4],
    length: usize,
}
pub(super) struct Decoded {
    pub(super) chars: [char; 4],
    pub(super) length: usize,
    pub(super) replacements: u64,
}
impl Default for Decoded {
    fn default() -> Self {
        Self { chars: ['\0'; 4], length: 0, replacements: 0 }
    }
}
impl Decoder {
    pub(super) fn push(&mut self, byte: u8) -> Decoded {
        self.bytes[self.length] = byte;
        self.length += 1;
        let mut result = Decoded::default();
        loop {
            match std::str::from_utf8(&self.bytes[..self.length]) {
                Ok(text) => {
                    for character in text.chars() {
                        result.chars[result.length] = character;
                        result.length += 1;
                    }
                    self.length = 0;
                    break;
                }
                Err(error) => {
                    let Some(count) = error.error_len() else { break };
                    // The retained prefix is always either empty or one incomplete scalar.
                    result.chars[result.length] = '\u{fffd}';
                    result.length += 1;
                    result.replacements += 1;
                    self.bytes.copy_within(count..self.length, 0);
                    self.length -= count;
                    if self.length == 0 {
                        break;
                    }
                }
            }
        }
        result
    }
    pub(super) fn finish(&mut self) -> Option<char> {
        let pending = self.length != 0;
        self.length = 0;
        pending.then_some('\u{fffd}')
    }
}
#[cfg(test)]
mod tests;
