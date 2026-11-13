// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Localized words for starting while a previous SugarSubstitute window is still open.
/// Complete copy for verified profile-conflict resolution.
#[non_exhaustive]
pub struct ConflictMessages {
    /// Native dialog heading.
    pub title: &'static str,
    /// Heading while this launch waits for the previous window to finish closing.
    pub starting: &'static str,
    /// Ownership discovery in progress.
    pub checking: &'static str,
    /// Explain that the displayed installation owns the profile.
    pub prompt: &'static str,
    /// Request ordinary application close and continue startup.
    pub close: &'static str,
    /// Leave the conflicting copy alone and cancel this launch.
    pub cancel: &'static str,
    /// Ordinary shutdown is still in progress.
    pub waiting: &'static str,
    /// Explicit warning accompanying the optional forced termination action.
    pub force_prompt: &'static str,
    /// Explicit forced termination action.
    pub force: &'static str,
    /// Wait for native ownership to retire after termination.
    pub cleanup: &'static str,
    /// Unverified or failed resolution never enables process control.
    pub failed: &'static str,
}
impl ConflictMessages {
    /// Enumerate all labels for locale coverage validation.
    pub fn labels(&self) -> [&'static str; 11] {
        [
            self.title,
            self.starting,
            self.checking,
            self.prompt,
            self.close,
            self.cancel,
            self.waiting,
            self.force_prompt,
            self.force,
            self.cleanup,
            self.failed,
        ]
    }
}
/// Complete EN conflict copy.
pub static EN: ConflictMessages = ConflictMessages {
    title: "SugarSubstitute isn't responding",
    starting: "Starting SugarSubstitute",
    checking: "Checking the previous SugarSubstitute window…",
    prompt: "The previous SugarSubstitute window isn't responding. Close it and start? Anything it hadn't saved may be lost.",
    close: "Close it and start",
    cancel: "Cancel",
    waiting: "Closing the previous window…",
    force_prompt: "The previous window still hasn't closed. Close it and start? Anything it hadn't saved may be lost.",
    force: "Close it and start",
    cleanup: "Finishing closing the previous window…",
    failed: "The previous SugarSubstitute window couldn't be closed. Close it yourself, then start SugarSubstitute again.",
};
/// Complete ZH_HANS conflict copy.
pub static ZH_HANS: ConflictMessages = ConflictMessages {
    title: "SugarSubstitute 未响应",
    starting: "正在启动 SugarSubstitute",
    checking: "正在检查之前的 SugarSubstitute 窗口…",
    prompt: "之前的 SugarSubstitute 窗口没有响应。要关闭它并启动吗？未保存的内容可能会丢失。",
    close: "关闭并启动",
    cancel: "取消",
    waiting: "正在关闭之前的窗口…",
    force_prompt: "之前的窗口仍未关闭。要关闭它并启动吗？未保存的内容可能会丢失。",
    force: "关闭并启动",
    cleanup: "正在完成关闭之前的窗口…",
    failed: "无法关闭之前的 SugarSubstitute 窗口。请手动关闭它，然后重新启动 SugarSubstitute。",
};
/// Complete JA conflict copy.
pub static JA: ConflictMessages = ConflictMessages {
    title: "SugarSubstitute が応答していません",
    starting: "SugarSubstitute を起動しています",
    checking: "前の SugarSubstitute ウィンドウを確認しています…",
    prompt: "前の SugarSubstitute ウィンドウが応答していません。閉じて起動しますか？保存されていない内容は失われる可能性があります。",
    close: "閉じて起動",
    cancel: "キャンセル",
    waiting: "前のウィンドウを閉じています…",
    force_prompt: "前のウィンドウがまだ閉じていません。閉じて起動しますか？保存されていない内容は失われる可能性があります。",
    force: "閉じて起動",
    cleanup: "前のウィンドウの終了を完了しています…",
    failed: "前の SugarSubstitute ウィンドウを閉じられませんでした。手動で閉じてから、SugarSubstitute をもう一度起動してください。",
};
/// Complete KO conflict copy.
pub static KO: ConflictMessages = ConflictMessages {
    title: "SugarSubstitute가 응답하지 않습니다",
    starting: "SugarSubstitute를 시작하는 중",
    checking: "이전 SugarSubstitute 창을 확인하는 중…",
    prompt: "이전 SugarSubstitute 창이 응답하지 않습니다. 닫고 시작할까요? 저장하지 않은 내용은 사라질 수 있습니다.",
    close: "닫고 시작",
    cancel: "취소",
    waiting: "이전 창을 닫는 중…",
    force_prompt: "이전 창이 아직 닫히지 않았습니다. 닫고 시작할까요? 저장하지 않은 내용은 사라질 수 있습니다.",
    force: "닫고 시작",
    cleanup: "이전 창을 마저 닫는 중…",
    failed: "이전 SugarSubstitute 창을 닫을 수 없습니다. 직접 닫은 다음 SugarSubstitute를 다시 시작하세요.",
};
/// Complete ES conflict copy.
pub static ES: ConflictMessages = ConflictMessages {
    title: "SugarSubstitute no responde",
    starting: "Iniciando SugarSubstitute",
    checking: "Comprobando la ventana anterior de SugarSubstitute…",
    prompt: "La ventana anterior de SugarSubstitute no responde. ¿Cerrarla e iniciar? Se puede perder lo que no se haya guardado.",
    close: "Cerrarla e iniciar",
    cancel: "Cancelar",
    waiting: "Cerrando la ventana anterior…",
    force_prompt: "La ventana anterior todavía no se ha cerrado. ¿Cerrarla e iniciar? Se puede perder lo que no se haya guardado.",
    force: "Cerrarla e iniciar",
    cleanup: "Terminando de cerrar la ventana anterior…",
    failed: "No se pudo cerrar la ventana anterior de SugarSubstitute. Ciérrala tú y vuelve a iniciar SugarSubstitute.",
};
