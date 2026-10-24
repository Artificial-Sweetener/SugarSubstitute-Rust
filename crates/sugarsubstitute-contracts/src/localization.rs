// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own SugarSubstitute's product copy and its sole supported-locale inventory.

/// Localized profile-conflict decisions.
pub mod conflict;
/// Localized native incident reporting copy.
pub mod incident;
/// Localized installer screens and actions.
pub mod installer;
/// Localized operation-specific startup feedback.
pub mod startup;
/// Localized native main-window controls.
pub mod window;

/// Bind one supported locale to every owned product surface.
/// Catalog construction belongs to this owner, including access through renamed imports.
/// ```compile_fail
/// use sugarsubstitute_contracts::localization::{Messages as Copy, messages};
/// let replacement = Copy { ..*messages("en") };
/// ```
#[non_exhaustive]
pub struct Messages {
    /// Native self-name displayed by language selection.
    pub name: &'static str,
    /// Complete installer copy.
    pub installer: &'static installer::InstallerMessages,
    /// Native profile-conflict controls and feedback.
    pub conflict: &'static conflict::ConflictMessages,
    /// Startup operation and long-wait labels.
    pub startup: &'static startup::StartupMessages,
    /// Locale identifier accepted by the executable.
    pub id: &'static str,
    /// Initial status and native window title.
    pub loading: &'static str,
    /// Feedback after two minutes of active startup.
    pub loading_long: &'static str,
    /// Feedback after five minutes of active startup.
    pub loading_extended: &'static str,
    /// Accessible intent for the collapsed output button.
    pub show_output: &'static str,
    /// Accessible intent for the expanded output button.
    pub hide_output: &'static str,
    /// Accessible name of the retained read-only backend output console.
    pub output: &'static str,
    /// Accessible intent for the close button.
    pub cancel: &'static str,
    /// Failed startup status.
    pub connection_failed: &'static str,
    /// Status while genuine original cleanup remains unresolved.
    pub cleanup_pending: &'static str,
    /// Status while failed cleanup retains actual ownership.
    pub cleanup_failed: &'static str,
    /// Actionable status after genuine cleanup settlement.
    pub retry_ready: &'static str,
    /// Accessible retry action.
    pub retry: &'static str,
    /// Explain a live profile exclusion conflict and how to open the requested copy.
    pub profile_in_use: &'static str,
    /// Complete native report surface copy.
    pub report: &'static incident::ReportMessages,
    /// Accessible names for the native main-window caption controls.
    pub window: &'static window::WindowMessages,
}

/// Keep the sole locale inventory together with complete owned-surface coverage.
pub static LOCALES: &[Messages] = &[
    Messages {
        id: "en",
        name: "English",
        installer: &installer::EN,
        startup: &startup::EN,
        loading: "Loading...",
        loading_long: "This is taking longer than usual...",
        loading_extended: "Still waiting. Dependencies or custom nodes\nmay take several minutes...",
        show_output: "Show Comfy output",
        hide_output: "Hide Comfy output",
        output: "Comfy output",
        cancel: "Cancel loading",
        connection_failed: "Connection failed",
        cleanup_pending: "Connection failed. Finishing cleanup...",
        cleanup_failed: "Cleanup failed. Retry is unavailable",
        retry_ready: "Connection failed. Select here to retry",
        retry: "Retry",
        profile_in_use: "This profile is already in use by another SugarSubstitute window or copy. Close that copy and let it finish shutting down, then open SugarSubstitute again. Your profile has not been changed.",
        report: &incident::EN,
        conflict: &conflict::EN,
        window: &window::EN,
    },
    Messages {
        id: "zh-Hans",
        name: "简体中文",
        installer: &installer::ZH_HANS,
        startup: &startup::ZH_HANS,
        loading: "正在加载...",
        loading_long: "加载时间比平时稍长...",
        loading_extended: "仍在等待。依赖项或自定义节点\n可能需要几分钟...",
        show_output: "显示 Comfy 输出",
        hide_output: "隐藏 Comfy 输出",
        output: "Comfy 输出",
        cancel: "取消加载",
        connection_failed: "连接失败",
        cleanup_pending: "连接失败，正在完成清理...",
        cleanup_failed: "清理失败，无法重试",
        retry_ready: "连接失败，点击此处重试",
        retry: "重试",
        profile_in_use: "另一个 SugarSubstitute 窗口或副本正在使用此配置文件。请关闭该副本并等待其完全退出，然后重新打开 SugarSubstitute。您的配置文件未被更改。",
        report: &incident::ZH_HANS,
        conflict: &conflict::ZH_HANS,
        window: &window::ZH_HANS,
    },
    Messages {
        id: "ja",
        name: "日本語",
        installer: &installer::JA,
        startup: &startup::JA,
        loading: "読み込み中...",
        loading_long: "通常より時間がかかっています...",
        loading_extended: "引き続き待機中です。依存関係やカスタムノードの\n準備には数分かかる場合があります...",
        show_output: "Comfy の出力を表示",
        hide_output: "Comfy の出力を非表示",
        output: "Comfy の出力",
        cancel: "読み込みをキャンセル",
        connection_failed: "接続に失敗しました",
        cleanup_pending: "接続に失敗しました。終了処理中...",
        cleanup_failed: "終了処理に失敗しました。再試行できません",
        retry_ready: "接続に失敗しました。ここを選択して再試行",
        retry: "再試行",
        profile_in_use: "このプロファイルは別の SugarSubstitute のウィンドウまたはコピーで使用中です。そのコピーを閉じて終了処理が完了してから、SugarSubstitute を開き直してください。プロファイルは変更されていません。",
        report: &incident::JA,
        conflict: &conflict::JA,
        window: &window::JA,
    },
    Messages {
        id: "ko",
        name: "한국어",
        installer: &installer::KO,
        startup: &startup::KO,
        loading: "불러오는 중...",
        loading_long: "평소보다 시간이 오래 걸리고 있습니다...",
        loading_extended: "계속 기다리는 중입니다. 의존성이나 사용자 정의 노드\n준비에 몇 분이 걸릴 수 있습니다...",
        show_output: "Comfy 출력 표시",
        hide_output: "Comfy 출력 숨기기",
        output: "Comfy 출력",
        cancel: "불러오기 취소",
        connection_failed: "연결 실패",
        cleanup_pending: "연결 실패. 정리 중...",
        cleanup_failed: "정리 실패. 다시 시도할 수 없습니다",
        retry_ready: "연결 실패. 여기를 선택하여 다시 시도",
        retry: "다시 시도",
        profile_in_use: "다른 SugarSubstitute 창 또는 복사본이 이 프로필을 사용 중입니다. 해당 복사본을 닫고 종료가 완료될 때까지 기다린 다음 SugarSubstitute를 다시 여세요. 프로필은 변경되지 않았습니다.",
        report: &incident::KO,
        conflict: &conflict::KO,
        window: &window::KO,
    },
    Messages {
        id: "es",
        name: "Español",
        installer: &installer::ES,
        startup: &startup::ES,
        loading: "Cargando...",
        loading_long: "Esto está tardando más de lo habitual...",
        loading_extended: "Seguimos esperando. Las dependencias o los nodos\npersonalizados pueden tardar varios minutos...",
        show_output: "Mostrar la salida de Comfy",
        hide_output: "Ocultar la salida de Comfy",
        output: "Salida de Comfy",
        cancel: "Cancelar la carga",
        connection_failed: "Error de conexión",
        cleanup_pending: "Error de conexión. Finalizando limpieza...",
        cleanup_failed: "Falló la limpieza. No se puede reintentar",
        retry_ready: "Error de conexión. Selecciona aquí para reintentar",
        retry: "Reintentar",
        profile_in_use: "Otra ventana o copia de SugarSubstitute está usando este perfil. Cierra esa copia y espera a que termine de cerrarse; después, vuelve a abrir SugarSubstitute. Tu perfil no se ha modificado.",
        report: &incident::ES,
        conflict: &conflict::ES,
        window: &window::ES,
    },
];

/// Resolve an exact locale, falling back to the canonical English record.
pub fn messages(locale: &str) -> &'static Messages {
    LOCALES.iter().find(|messages| messages.id == locale).unwrap_or(&LOCALES[0])
}

#[cfg(test)]
mod tests {
    use super::{LOCALES, messages};

    /// Require unique registry identities and complete visible/control text.
    #[test]
    fn locale_registry_is_complete_and_has_canonical_fallback() {
        let mut identities = std::collections::BTreeSet::new();
        for locale in LOCALES {
            assert!(identities.insert(locale.id));
            assert!(
                [
                    locale.loading,
                    locale.loading_long,
                    locale.loading_extended,
                    locale.show_output,
                    locale.hide_output,
                    locale.output,
                    locale.cancel,
                    locale.connection_failed,
                    locale.cleanup_pending,
                    locale.cleanup_failed,
                    locale.retry_ready,
                    locale.retry,
                    locale.profile_in_use
                ]
                .into_iter()
                .chain([locale.name])
                .chain(locale.installer.labels())
                .chain(locale.report.labels())
                .chain(locale.conflict.labels())
                .chain(locale.window.labels())
                .chain(locale.startup.labels())
                .all(|text| !text.trim().is_empty() && !text.contains('\0'))
            );
        }
        assert_eq!(messages("unrecognized").id, "en");
    }
}
