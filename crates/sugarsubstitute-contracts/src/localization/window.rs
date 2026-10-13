// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own accessible native caption names selected by the sole supported-locale registry.

/// Names used by native main-window controls independently of web-owned content.
pub struct WindowMessages {
    /// Minimize the application window.
    pub minimize: &'static str,
    /// Maximize the application window.
    pub maximize: &'static str,
    /// Restore the application window's previous bounds.
    pub restore: &'static str,
    /// Close the application window.
    pub close: &'static str,
}

impl WindowMessages {
    /// Enumerate every owned caption label for complete registry coverage checks.
    pub const fn labels(&self) -> [&'static str; 4] {
        [self.minimize, self.maximize, self.restore, self.close]
    }
}

/// Canonical English native caption names.
pub static EN: WindowMessages = WindowMessages {
    minimize: "Minimize",
    maximize: "Maximize",
    restore: "Restore",
    close: "Close",
};

/// Simplified Chinese native caption names.
pub static ZH_HANS: WindowMessages =
    WindowMessages {
        minimize: "最小化", maximize: "最大化", restore: "还原", close: "关闭"
    };

/// Japanese native caption names.
pub static JA: WindowMessages = WindowMessages {
    minimize: "最小化",
    maximize: "最大化",
    restore: "元のサイズに戻す",
    close: "閉じる",
};

/// Korean native caption names.
pub static KO: WindowMessages = WindowMessages {
    minimize: "최소화",
    maximize: "최대화",
    restore: "이전 크기로 복원",
    close: "닫기",
};

/// Spanish native caption names.
pub static ES: WindowMessages = WindowMessages {
    minimize: "Minimizar",
    maximize: "Maximizar",
    restore: "Restaurar",
    close: "Cerrar",
};
