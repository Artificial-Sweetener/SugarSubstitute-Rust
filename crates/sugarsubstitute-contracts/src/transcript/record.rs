// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expose immutable foreign text and UTF-8 byte style ranges through borrowed access.
use super::{MAX_RECORD_BYTES, MAX_RECORD_RUNS, OutputStream, TranscriptError};
use std::{fmt, num::NonZeroU64};

/// Stable record identity within one resource generation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RecordId(NonZeroU64);
impl RecordId {
    /// Reject zero before retention.
    pub const fn new(value: u64) -> Result<Self, TranscriptError> {
        match NonZeroU64::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(TranscriptError::ZeroIdentity),
        }
    }
    /// Return the generation-local record number.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}
/// Supported ANSI color facts; palette interpretation belongs to presentation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AnsiColor {
    /// Theme-selected default color.
    #[default]
    Default,
    /// ANSI 256-color palette index; indices 0..15 include bright basic colors.
    Indexed(u8),
    /// Exact 24-bit RGB components.
    Rgb(u8, u8, u8),
}
/// Supported SGR color and bold state, independent of font/native types.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextStyle {
    /// Text color.
    pub foreground: AnsiColor,
    /// Background color.
    pub background: AnsiColor,
    /// Bold emphasis.
    pub bold: bool,
}
/// One nonempty, half-open UTF-8 byte range within a record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StyleRun {
    /// Inclusive UTF-8 byte start.
    pub start: u32,
    /// Exclusive UTF-8 byte end.
    pub end: u32,
    /// Supported style throughout the range.
    pub style: TextStyle,
}
/// Immutable record; consumers borrow content rather than cloning retained ownership.
pub struct TranscriptRecord {
    id: RecordId,
    stream: OutputStream,
    text: String,
    runs: Vec<StyleRun>,
    complete: bool,
}
impl fmt::Debug for TranscriptRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TranscriptRecord")
            .field("id", &self.id)
            .field("stream", &self.stream)
            .field("text_bytes", &self.text.len())
            .field("style_runs", &self.runs.len())
            .field("complete", &self.complete)
            .finish()
    }
}
impl TranscriptRecord {
    /// Move bounded text/runs after validating exact nonoverlapping UTF-8 coverage.
    /// Empty records require no runs. Newline delimiters are not retained as text.
    pub fn new(
        id: RecordId,
        stream: OutputStream,
        text: String,
        runs: Vec<StyleRun>,
        complete: bool,
    ) -> Result<Self, TranscriptError> {
        if text.len() > MAX_RECORD_BYTES
            || text.capacity() > MAX_RECORD_BYTES
            || runs.len() > MAX_RECORD_RUNS
            || runs.capacity() > MAX_RECORD_RUNS
        {
            return Err(TranscriptError::Bound);
        }
        let mut end = 0;
        for run in &runs {
            let next = run.end as usize;
            if run.start as usize != end
                || next <= end
                || next > text.len()
                || !text.is_char_boundary(end)
                || !text.is_char_boundary(next)
            {
                return Err(TranscriptError::InvalidStyle);
            }
            end = next;
        }
        if end != text.len() {
            return Err(TranscriptError::InvalidStyle);
        }
        Ok(Self { id, stream, text, runs, complete })
    }
    /// Return the stable selection/scroll identity.
    pub const fn id(&self) -> RecordId {
        self.id
    }
    /// Return the originating stream.
    pub const fn stream(&self) -> OutputStream {
        self.stream
    }
    /// Borrow exact retained foreign UTF-8 text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Borrow validated byte ranges.
    pub fn runs(&self) -> &[StyleRun] {
        &self.runs
    }
    /// Return whether a delimiter, EOF or gap sealed this record.
    pub const fn complete(&self) -> bool {
        self.complete
    }
    /// Return observable retained string/run capacities, in bytes.
    pub fn allocated_bytes(&self) -> usize {
        self.text.capacity() + self.runs.capacity() * size_of::<StyleRun>()
    }
}
