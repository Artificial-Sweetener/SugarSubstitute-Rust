// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the close save's window state and the next start's one-time notice of unsaved work.

/// Copy shown while the window stays up to save, and after a close that did not keep
/// everything.
#[non_exhaustive]
pub struct ClosingMessages {
    /// Shown while the workspace and input canvas save before the application closes.
    pub saving: &'static str,
    /// Next-start notice when only workflow changes were lost.
    pub not_saved_workspace: &'static str,
    /// Next-start notice when only input canvas edits were lost.
    pub not_saved_inputs: &'static str,
    /// Next-start notice when both were lost.
    pub not_saved_both: &'static str,
}

impl ClosingMessages {
    /// Enumerate every owned label for complete registry coverage checks.
    pub const fn labels(&self) -> [&'static str; 4] {
        [self.saving, self.not_saved_workspace, self.not_saved_inputs, self.not_saved_both]
    }

    /// The next-start notice naming what the previous close lost, or `None` when it kept both.
    pub const fn not_saved(&self, workspace: bool, inputs: bool) -> Option<&'static str> {
        match (workspace, inputs) {
            (true, true) => Some(self.not_saved_both),
            (true, false) => Some(self.not_saved_workspace),
            (false, true) => Some(self.not_saved_inputs),
            (false, false) => None,
        }
    }
}

/// Canonical English closing copy.
pub static EN: ClosingMessages = ClosingMessages {
    saving: "Saving your workspace…",
    not_saved_workspace: "Your latest workflow changes were not saved when SugarSubstitute last closed.",
    not_saved_inputs: "Your latest input canvas edits were not saved when SugarSubstitute last closed.",
    not_saved_both: "Your latest workflow changes and input canvas edits were not saved when SugarSubstitute last closed.",
};

/// Simplified Chinese closing copy.
pub static ZH_HANS: ClosingMessages = ClosingMessages {
    saving: "正在保存工作区…",
    not_saved_workspace: "SugarSubstitute 上次关闭时，最近的工作流更改未能保存。",
    not_saved_inputs: "SugarSubstitute 上次关闭时，最近的输入画布编辑未能保存。",
    not_saved_both: "SugarSubstitute 上次关闭时，最近的工作流更改和输入画布编辑未能保存。",
};

/// Japanese closing copy.
pub static JA: ClosingMessages = ClosingMessages {
    saving: "ワークスペースを保存しています…",
    not_saved_workspace: "前回 SugarSubstitute を閉じたとき、最新のワークフローの変更は保存されませんでした。",
    not_saved_inputs: "前回 SugarSubstitute を閉じたとき、入力キャンバスの最新の編集は保存されませんでした。",
    not_saved_both: "前回 SugarSubstitute を閉じたとき、最新のワークフローの変更と入力キャンバスの編集は保存されませんでした。",
};

/// Korean closing copy.
pub static KO: ClosingMessages = ClosingMessages {
    saving: "작업 공간을 저장하는 중…",
    not_saved_workspace: "SugarSubstitute를 마지막으로 닫을 때 최근 워크플로 변경 사항이 저장되지 않았습니다.",
    not_saved_inputs: "SugarSubstitute를 마지막으로 닫을 때 최근 입력 캔버스 편집 내용이 저장되지 않았습니다.",
    not_saved_both: "SugarSubstitute를 마지막으로 닫을 때 최근 워크플로 변경 사항과 입력 캔버스 편집 내용이 저장되지 않았습니다.",
};

/// Spanish closing copy.
pub static ES: ClosingMessages = ClosingMessages {
    saving: "Guardando tu espacio de trabajo…",
    not_saved_workspace: "Los últimos cambios del flujo de trabajo no se guardaron la última vez que se cerró SugarSubstitute.",
    not_saved_inputs: "Las últimas ediciones del lienzo de entrada no se guardaron la última vez que se cerró SugarSubstitute.",
    not_saved_both: "Los últimos cambios del flujo de trabajo y las ediciones del lienzo de entrada no se guardaron la última vez que se cerró SugarSubstitute.",
};
