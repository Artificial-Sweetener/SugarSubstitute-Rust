// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Localized decisions for an occupied desktop profile.
/// Complete copy for verified profile-conflict resolution.
pub struct ConflictMessages {
    /// Native dialog heading.
    pub title: &'static str,
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
    pub fn labels(&self) -> [&'static str; 10] {
        [
            self.title,
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
    title: "Profile already in use",
    checking: "Checking which copy is using this profile…",
    prompt: "This copy of SugarSubstitute is using the profile. You can close it and continue launching:",
    close: "Close other copy and continue",
    cancel: "Cancel this launch",
    waiting: "Waiting for the other copy to close…",
    force_prompt: "The other copy has not finished closing. You can keep waiting or force quit it. Force quitting may lose unsaved work.",
    force: "Force quit other copy",
    cleanup: "Waiting for the other copy and its owned processes to exit…",
    failed: "The conflicting copy could not be safely closed or its ownership could not be verified. Close it manually, then launch SugarSubstitute again.",
};
/// Complete ZH_HANS conflict copy.
pub static ZH_HANS: ConflictMessages = ConflictMessages {
    title: "配置文件正在使用中",
    checking: "正在检查哪个副本使用此配置文件…",
    prompt: "此 SugarSubstitute 副本正在使用该配置文件。您可以关闭它并继续启动：",
    close: "关闭其他副本并继续",
    cancel: "取消此次启动",
    waiting: "正在等待其他副本关闭…",
    force_prompt: "其他副本尚未完成关闭。您可以继续等待或强制退出。强制退出可能导致未保存的工作丢失。",
    force: "强制退出其他副本",
    cleanup: "正在等待其他副本及其所属进程退出…",
    failed: "无法安全关闭冲突的副本或验证其所有权。请手动关闭它，然后重新启动 SugarSubstitute。",
};
/// Complete JA conflict copy.
pub static JA: ConflictMessages = ConflictMessages {
    title: "プロファイルは使用中です",
    checking: "このプロファイルを使用しているコピーを確認しています…",
    prompt: "この SugarSubstitute のコピーがプロファイルを使用しています。閉じて起動を続けられます：",
    close: "別のコピーを閉じて続行",
    cancel: "今回の起動をキャンセル",
    waiting: "別のコピーの終了を待っています…",
    force_prompt: "別のコピーの終了が完了していません。待機を続けるか、強制終了できます。強制終了すると未保存の作業が失われる可能性があります。",
    force: "別のコピーを強制終了",
    cleanup: "別のコピーとその管理下のプロセスの終了を待っています…",
    failed: "競合するコピーを安全に終了できないか、所有関係を確認できませんでした。手動で閉じてから SugarSubstitute を起動し直してください。",
};
/// Complete KO conflict copy.
pub static KO: ConflictMessages = ConflictMessages {
    title: "프로필 사용 중",
    checking: "이 프로필을 사용하는 복사본을 확인하는 중…",
    prompt: "이 SugarSubstitute 복사본이 프로필을 사용 중입니다. 닫고 실행을 계속할 수 있습니다:",
    close: "다른 복사본을 닫고 계속",
    cancel: "이번 실행 취소",
    waiting: "다른 복사본이 종료되기를 기다리는 중…",
    force_prompt: "다른 복사본의 종료가 완료되지 않았습니다. 계속 기다리거나 강제로 종료할 수 있습니다. 강제 종료하면 저장하지 않은 작업이 손실될 수 있습니다.",
    force: "다른 복사본 강제 종료",
    cleanup: "다른 복사본과 해당 복사본이 관리하는 프로세스가 종료되기를 기다리는 중…",
    failed: "충돌하는 복사본을 안전하게 종료하거나 소유 관계를 확인할 수 없습니다. 수동으로 닫은 다음 SugarSubstitute를 다시 실행하세요.",
};
/// Complete ES conflict copy.
pub static ES: ConflictMessages = ConflictMessages {
    title: "Perfil en uso",
    checking: "Comprobando qué copia está usando este perfil…",
    prompt: "Esta copia de SugarSubstitute está usando el perfil. Puedes cerrarla y continuar el inicio:",
    close: "Cerrar la otra copia y continuar",
    cancel: "Cancelar este inicio",
    waiting: "Esperando a que se cierre la otra copia…",
    force_prompt: "La otra copia no ha terminado de cerrarse. Puedes seguir esperando o forzar su cierre. Forzar el cierre puede perder el trabajo sin guardar.",
    force: "Forzar el cierre de la otra copia",
    cleanup: "Esperando a que salgan la otra copia y los procesos que administra…",
    failed: "No se pudo cerrar de forma segura la copia en conflicto o verificar su propiedad. Ciérrala manualmente y vuelve a iniciar SugarSubstitute.",
};
