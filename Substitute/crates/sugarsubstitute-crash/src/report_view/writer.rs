// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep all formatting inside one fixed claimed allocation with explicit omission.
use super::ReportViewError;
use std::fmt::{self, Write};
pub(super) struct Writer {
    text: String,
    bound: usize,
    marker: &'static str,
    omitted: bool,
}
impl Writer {
    pub(super) fn new(bound: usize, marker: &'static str) -> Result<Self, ReportViewError> {
        if marker.len().checked_add(2).is_none_or(|minimum| minimum > bound) {
            return Err(ReportViewError::Bounds);
        }
        let mut text = String::new();
        text.try_reserve_exact(bound).map_err(ReportViewError::Allocation)?;
        if text.capacity() > bound {
            return Err(ReportViewError::Bounds);
        }
        Ok(Self { text, bound, marker, omitted: false })
    }
    pub(super) fn line(&mut self, text: &str) {
        self.bytes(text.as_bytes());
        self.bytes(b"\n");
    }
    pub(super) fn line_args(&mut self, args: fmt::Arguments<'_>) {
        // The bounded writer never returns an error and cannot grow its allocation.
        if self.write_fmt(args).is_err() {
            self.omitted = true;
        }
        self.bytes(b"\n");
    }
    pub(super) fn named(&mut self, label: &str, args: fmt::Arguments<'_>) {
        self.bytes(label.as_bytes());
        self.bytes(b": ");
        self.line_args(args);
    }
    pub(super) fn bytes(&mut self, bytes: &[u8]) {
        let Ok(text) = std::str::from_utf8(bytes) else {
            self.omitted = true;
            return;
        };
        if self.omitted {
            return;
        }
        let available = self.bound - self.marker.len() - 2 - self.text.len();
        if text.len() <= available {
            self.text.push_str(text);
        } else {
            let mut end = available.min(text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            self.text.push_str(&text[..end]);
            self.omitted = true;
        }
    }
    pub(super) fn finish(mut self) -> String {
        if !self.text.ends_with('\n') {
            self.text.push('\n');
        }
        if self.omitted {
            self.text.push_str(self.marker);
            self.text.push('\n');
        }
        self.text
    }
}
impl Write for Writer {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.bytes(text.as_bytes());
        Ok(())
    }
}
