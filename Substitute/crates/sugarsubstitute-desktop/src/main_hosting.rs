// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Name the browser hosting that main-window construction actually selected.

/// How the main WebView is hosted for one native main lifetime.
///
/// The desktop fixes this before the WebView is constructed. It is a native fact attached
/// to page-originated observations; page content cannot choose or change it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainHosting {
    /// The browser renders into an owned DirectComposition visual tree, so native canvas
    /// pixels can be composed beside it.
    Composition,
    /// The browser is an ordinary windowed child. Native canvas presentation is unavailable
    /// and the page presents its canvas itself.
    Windowed,
}

impl MainHosting {
    /// Report whether native canvas presentation can be granted to this main lifetime.
    pub const fn presents_native_canvas(self) -> bool {
        matches!(self, Self::Composition)
    }
}
