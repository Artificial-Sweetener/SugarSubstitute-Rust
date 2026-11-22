// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Name the text edit rows native controls offer beside the page's own text menus.

/// Edit row names, in the order every text menu of the application shows them.
#[non_exhaustive]
pub struct EditMessages {
    /// Undo the last edit.
    pub undo: &'static str,
    /// Redo the last undone edit.
    pub redo: &'static str,
    /// Move the selected text to the clipboard.
    pub cut: &'static str,
    /// Copy the selected text to the clipboard.
    pub copy: &'static str,
    /// Insert the clipboard's text.
    pub paste: &'static str,
    /// Select all text.
    pub select_all: &'static str,
}

impl EditMessages {
    /// Enumerate every edit row name for complete registry coverage checks.
    pub const fn labels(&self) -> [&'static str; 6] {
        [self.undo, self.redo, self.cut, self.copy, self.paste, self.select_all]
    }
}

/// Canonical English edit row names.
pub static EN: EditMessages = EditMessages {
    undo: "Undo",
    redo: "Redo",
    cut: "Cut",
    copy: "Copy",
    paste: "Paste",
    select_all: "Select all",
};

/// Simplified Chinese edit row names.
pub static ZH_HANS: EditMessages = EditMessages {
    undo: "撤销",
    redo: "重做",
    cut: "剪切",
    copy: "复制",
    paste: "粘贴",
    select_all: "全选",
};

/// Japanese edit row names.
pub static JA: EditMessages = EditMessages {
    undo: "元に戻す",
    redo: "やり直し",
    cut: "切り取り",
    copy: "コピー",
    paste: "貼り付け",
    select_all: "すべて選択",
};

/// Korean edit row names.
pub static KO: EditMessages = EditMessages {
    undo: "실행 취소",
    redo: "다시 실행",
    cut: "잘라내기",
    copy: "복사",
    paste: "붙여넣기",
    select_all: "모두 선택",
};

/// Spanish edit row names.
pub static ES: EditMessages = EditMessages {
    undo: "Deshacer",
    redo: "Rehacer",
    cut: "Cortar",
    copy: "Copiar",
    paste: "Pegar",
    select_all: "Seleccionar todo",
};
