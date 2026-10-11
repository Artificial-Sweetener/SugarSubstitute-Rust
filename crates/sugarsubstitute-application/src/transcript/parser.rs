// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep byte decoding and escape/style state independent for each stream.
use super::{
    ansi::Ansi,
    utf8::{Decoded, Decoder},
};
use sugarsubstitute_contracts::transcript::TextStyle;
#[derive(Default)]
pub(super) struct Parser {
    ansi: Ansi,
    decoder: Decoder,
}
impl Parser {
    pub(super) fn push(&mut self, byte: u8) -> (Decoded, u64) {
        // A control boundary cannot complete an interrupted raw UTF-8 scalar.
        let interrupted = if byte == 0x1b { self.decoder.finish() } else { None };
        let (visible, malformed) = self.ansi.push(byte);
        let mut decoded = visible.map_or_else(Decoded::default, |b| self.decoder.push(b));
        if let Some(character) = interrupted {
            decoded.chars.copy_within(0..decoded.length, 1);
            decoded.chars[0] = character;
            decoded.length += 1;
            decoded.replacements += 1;
        }
        (decoded, malformed)
    }
    pub(super) fn style(&self) -> TextStyle {
        self.ansi.style
    }
    pub(super) fn finish(&mut self) -> (Option<char>, u64) {
        (self.decoder.finish(), self.ansi.finish())
    }
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }
}
