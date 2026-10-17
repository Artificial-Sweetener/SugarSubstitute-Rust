// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow one immutable console view without granting process or scheduler authority.
use crate::ControlBounds;
#[cfg(any(windows, test))]
pub(crate) mod focus;
#[cfg(any(windows, test))]
pub(crate) mod funding;
#[cfg(any(windows, test))]
pub(crate) mod index;
#[cfg(any(windows, test))]
pub(crate) mod update;
use sugarsubstitute_contracts::{lifecycle::ResourceKey, transcript::TranscriptRecord};
/// Borrow exact bounded foreign text; the composing content retains its source custody.
/// Calls stay on the native event-loop thread and must return the same immutable
/// records for the entire borrow. Records are ordered by stable, increasing ID.
pub trait ConsoleText {
    /// Exact resource generation, never inferred from display text.
    fn resource(&self) -> ResourceKey;
    /// Accepted source revision within that resource.
    fn revision(&self) -> u64;
    /// Number of records, at most the shared snapshot bound.
    fn len(&self) -> usize;
    /// Whether the retained view contains no records.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Borrow an indexed immutable record; no allocation or source-pin extraction.
    fn record(&self, index: usize) -> Option<&TranscriptRecord>;
}
/// Present one retained read-only console peer; hiding preserves its native document,
/// selection and viewport. Copy exposes only text selected by the user's native action.
pub struct ConsoleView<'a> {
    /// Stable localized accessible name, independent of foreign document contents.
    pub label: &'static str,
    /// Logical client rectangle matching the console's existing inner area.
    pub bounds: ControlBounds,
    /// Native control is available only after the matching expanded frame publishes.
    pub visible: bool,
    /// Select the same palette as the raster owner.
    pub dark: bool,
    /// Latest accepted transcript pin, borrowed with its original source custody.
    /// Text revisions are independent of RGBA preparation; the adapter coalesces copying
    /// until a newly funded frame is available, while retained exposures reuse its document.
    pub text: Option<&'a dyn ConsoleText>,
}
