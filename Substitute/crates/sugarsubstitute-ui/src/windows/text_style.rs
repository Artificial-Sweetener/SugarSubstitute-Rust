// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authoritative native text-control typography and editing treatment.
use windows::Win32::{
    Foundation::RECT,
    UI::{
        Controls::RichEdit::*,
        WindowsAndMessaging::{ES_AUTOHSCROLL, WS_VSCROLL},
    },
};

/// Shared native text roles; the desktop adapter owns documents and affine HWND lifetimes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStyle {
    /// Read-only, wrapping output with the established console typography and insets.
    Transcript,
    /// Editable single-line Fluent field with native selection, undo and horizontal scrolling.
    Field,
}

impl TextStyle {
    /// Native initial character format, in twips, retained for the document's lifetime.
    pub fn font(self) -> CHARFORMAT2W {
        let mut font = CHARFORMAT2W::default();
        font.Base.cbSize = size_of::<CHARFORMAT2W>() as u32;
        font.Base.dwMask = CFM_FACE | CFM_SIZE | CFM_COLOR | CFM_BACKCOLOR;
        font.Base.dwEffects = CFE_AUTOBACKCOLOR;
        let (family, height) = match self {
            Self::Transcript => ("Courier New", 180),
            Self::Field => ("Segoe UI", 210),
        };
        font.Base.yHeight = height;
        if self == Self::Field {
            // Plain editable fields resolve automatic foreground through their paint host.
            font.Base.dwEffects |= CFE_AUTOCOLOR;
        }
        for (slot, unit) in font.Base.szFaceName.iter_mut().zip(family.encode_utf16()) {
            *slot = unit;
        }
        font
    }
    /// Native property bits; fields retain plain text and never accept rich formatting from paste.
    pub fn properties(self) -> u32 {
        match self {
            Self::Transcript => {
                TXTBIT_RICHTEXT
                    | TXTBIT_MULTILINE
                    | TXTBIT_READONLY
                    | TXTBIT_WORDWRAP
                    | TXTBIT_D2DDWRITE
            }
            Self::Field => TXTBIT_D2DDWRITE,
        }
    }
    /// Native view insets in HIMETRIC; field bounds already exclude shared Fluent decoration.
    pub fn inset(self) -> RECT {
        match self {
            Self::Transcript => RECT { left: 265, top: 0, right: 265, bottom: 53 },
            Self::Field => RECT::default(),
        }
    }
    /// Maximum document size in UTF-16 units before native admission stops further input.
    pub const fn maximum_length(self) -> u32 {
        match self {
            Self::Transcript => {
                (sugarsubstitute_contracts::transcript::MAX_SNAPSHOT_BYTES
                    + sugarsubstitute_contracts::transcript::MAX_SNAPSHOT_RECORDS)
                    as u32
            }
            Self::Field => 4096,
        }
    }
    /// Native scrolling style; single-line fields have no visible scrollbar.
    pub fn scroll_bars(self) -> u32 {
        match self {
            Self::Transcript => WS_VSCROLL.0,
            Self::Field => ES_AUTOHSCROLL as u32,
        }
    }
    /// Bounded native edit history; transcript replacement never accumulates undo records.
    pub fn undo_limit(self) -> usize {
        match self {
            Self::Transcript => 0,
            Self::Field => 100,
        }
    }
}
