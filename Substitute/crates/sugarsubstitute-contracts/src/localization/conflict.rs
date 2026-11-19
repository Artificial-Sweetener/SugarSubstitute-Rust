// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Localized words for starting while a previous SugarSubstitute window is still open, and for
//! starting while a ComfyUI the application did not start holds its port.
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
/// Complete EN copy for a ComfyUI that already holds the application's port.
pub static BACKEND_EN: ConflictMessages = ConflictMessages {
    title: "ComfyUI is already running",
    starting: "Starting SugarSubstitute",
    checking: "Checking what is using ComfyUI's port…",
    prompt: "ComfyUI is already running on the port SugarSubstitute uses. SugarSubstitute needs to start and control ComfyUI itself to work correctly. Close ComfyUI and continue? Anything it is still running will be stopped.",
    close: "Close ComfyUI and continue",
    cancel: "Cancel",
    waiting: "Closing ComfyUI…",
    force_prompt: "ComfyUI still hasn't closed. Close ComfyUI and continue? Anything it is still running will be stopped.",
    force: "Close ComfyUI and continue",
    cleanup: "Finishing closing ComfyUI…",
    failed: "Something is using ComfyUI's port, and SugarSubstitute couldn't confirm it is ComfyUI or couldn't close it, so it left it alone. Close it yourself, then start SugarSubstitute again.",
};
/// Complete ZH_HANS copy for a ComfyUI that already holds the application's port.
pub static BACKEND_ZH_HANS: ConflictMessages = ConflictMessages {
    title: "ComfyUI 已在运行",
    starting: "正在启动 SugarSubstitute",
    checking: "正在检查占用 ComfyUI 端口的程序…",
    prompt: "ComfyUI 已在 SugarSubstitute 使用的端口上运行。SugarSubstitute 需要自行启动并控制 ComfyUI 才能正常工作。要关闭 ComfyUI 并继续吗？它正在运行的任务将被停止。",
    close: "关闭 ComfyUI 并继续",
    cancel: "取消",
    waiting: "正在关闭 ComfyUI…",
    force_prompt: "ComfyUI 仍未关闭。要关闭 ComfyUI 并继续吗？它正在运行的任务将被停止。",
    force: "关闭 ComfyUI 并继续",
    cleanup: "正在完成关闭 ComfyUI…",
    failed: "有程序占用了 ComfyUI 的端口，SugarSubstitute 无法确认它是 ComfyUI 或无法将其关闭，因此未作处理。请手动关闭它，然后重新启动 SugarSubstitute。",
};
/// Complete JA copy for a ComfyUI that already holds the application's port.
pub static BACKEND_JA: ConflictMessages = ConflictMessages {
    title: "ComfyUI はすでに実行中です",
    starting: "SugarSubstitute を起動しています",
    checking: "ComfyUI のポートを使用しているプログラムを確認しています…",
    prompt: "SugarSubstitute が使用するポートで ComfyUI がすでに実行されています。SugarSubstitute が正しく動作するには、ComfyUI を自ら起動して制御する必要があります。ComfyUI を閉じて続行しますか？実行中の処理は停止されます。",
    close: "ComfyUI を閉じて続行",
    cancel: "キャンセル",
    waiting: "ComfyUI を閉じています…",
    force_prompt: "ComfyUI がまだ閉じていません。ComfyUI を閉じて続行しますか？実行中の処理は停止されます。",
    force: "ComfyUI を閉じて続行",
    cleanup: "ComfyUI の終了を完了しています…",
    failed: "ComfyUI のポートが使用されていますが、SugarSubstitute はそれが ComfyUI であることを確認できなかったか、閉じることができなかったため、そのままにしました。手動で閉じてから、SugarSubstitute をもう一度起動してください。",
};
/// Complete KO copy for a ComfyUI that already holds the application's port.
pub static BACKEND_KO: ConflictMessages = ConflictMessages {
    title: "ComfyUI가 이미 실행 중입니다",
    starting: "SugarSubstitute를 시작하는 중",
    checking: "ComfyUI 포트를 사용 중인 프로그램을 확인하는 중…",
    prompt: "SugarSubstitute가 사용하는 포트에서 ComfyUI가 이미 실행 중입니다. SugarSubstitute가 올바르게 작동하려면 ComfyUI를 직접 시작하고 제어해야 합니다. ComfyUI를 닫고 계속할까요? 실행 중인 작업은 중지됩니다.",
    close: "ComfyUI를 닫고 계속",
    cancel: "취소",
    waiting: "ComfyUI를 닫는 중…",
    force_prompt: "ComfyUI가 아직 닫히지 않았습니다. ComfyUI를 닫고 계속할까요? 실행 중인 작업은 중지됩니다.",
    force: "ComfyUI를 닫고 계속",
    cleanup: "ComfyUI를 마저 닫는 중…",
    failed: "ComfyUI 포트를 다른 프로그램이 사용 중이지만, SugarSubstitute가 그것이 ComfyUI인지 확인하지 못했거나 닫지 못해 그대로 두었습니다. 직접 닫은 다음 SugarSubstitute를 다시 시작하세요.",
};
/// Complete ES copy for a ComfyUI that already holds the application's port.
pub static BACKEND_ES: ConflictMessages = ConflictMessages {
    title: "ComfyUI ya se está ejecutando",
    starting: "Iniciando SugarSubstitute",
    checking: "Comprobando qué está usando el puerto de ComfyUI…",
    prompt: "ComfyUI ya se está ejecutando en el puerto que usa SugarSubstitute. SugarSubstitute necesita iniciar y controlar ComfyUI por sí mismo para funcionar correctamente. ¿Cerrar ComfyUI y continuar? Se detendrá lo que todavía esté ejecutando.",
    close: "Cerrar ComfyUI y continuar",
    cancel: "Cancelar",
    waiting: "Cerrando ComfyUI…",
    force_prompt: "ComfyUI todavía no se ha cerrado. ¿Cerrar ComfyUI y continuar? Se detendrá lo que todavía esté ejecutando.",
    force: "Cerrar ComfyUI y continuar",
    cleanup: "Terminando de cerrar ComfyUI…",
    failed: "Algo está usando el puerto de ComfyUI, y SugarSubstitute no pudo confirmar que sea ComfyUI o no pudo cerrarlo, así que lo dejó como estaba. Ciérralo tú y vuelve a iniciar SugarSubstitute.",
};
