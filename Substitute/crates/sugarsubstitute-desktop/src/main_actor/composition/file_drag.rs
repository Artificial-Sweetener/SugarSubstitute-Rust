// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hand the main actor what the window's drop target learned about a file drag.

use super::MainActor;
use sugarsubstitute_contracts::host_file_drag::FileDragHostMessage;

impl MainActor {
    /// Take the newest unsent word about a file drag over the window: its files' names, or
    /// its end. A windowed browser takes its drags itself, so it never has one.
    pub(in crate::main_actor) fn take_file_drag(&self) -> Option<FileDragHostMessage> {
        self.composition.as_ref()?.input.as_ref()?.take_file_drag()
    }
}
