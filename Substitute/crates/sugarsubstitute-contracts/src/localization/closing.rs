// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the close save's window state.

/// Copy shown while the window stays up to save.
#[non_exhaustive]
pub struct ClosingMessages {
    /// Shown while the workspace and input canvas save before the application closes.
    pub saving: &'static str,
}

impl ClosingMessages {
    /// Enumerate every owned label for complete registry coverage checks.
    pub const fn labels(&self) -> [&'static str; 1] {
        [self.saving]
    }
}

/// Canonical English closing copy.
pub static EN: ClosingMessages = ClosingMessages { saving: "Saving your workspace…" };

/// Simplified Chinese closing copy.
pub static ZH_HANS: ClosingMessages = ClosingMessages { saving: "正在保存工作区…" };

/// Japanese closing copy.
pub static JA: ClosingMessages =
    ClosingMessages { saving: "ワークスペースを保存しています…" };

/// Korean closing copy.
pub static KO: ClosingMessages = ClosingMessages { saving: "작업 공간을 저장하는 중…" };

/// Spanish closing copy.
pub static ES: ClosingMessages = ClosingMessages { saving: "Guardando tu espacio de trabajo…" };
