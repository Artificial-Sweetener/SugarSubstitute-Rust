// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the words of the window's update controls, the update report's steps and the start-up
//! update messages.
use crate::update::attempt::AttemptPhase;

/// Placeholder that [`UpdateMessages::with_version`] replaces with a version number.
pub const VERSION: &str = "{version}";

/// Words for the update button, the restart offer and the start-up update lines.
#[non_exhaustive]
pub struct UpdateMessages {
    /// Name of the update button when an update is ready to apply.
    pub ready: &'static str,
    /// Name of the update button when the last update attempt failed.
    pub failed: &'static str,
    /// Restart now so the ready update applies.
    pub restart: &'static str,
    /// Restart once the running workflow finishes so the ready update applies.
    pub restart_after_workflow: &'static str,
    /// Hide a one-time message.
    pub dismiss: &'static str,
    /// Open the release notes of the version now running.
    pub whats_new: &'static str,
    /// Splash line while the new version is being applied; contains [`VERSION`].
    pub applying: &'static str,
    /// First start on a new version; contains [`VERSION`].
    pub updated: &'static str,
    /// The new version did not start, so the one before it runs.
    pub previous_running: &'static str,
    /// Update report step names in [`AttemptPhase`] declaration order.
    pub phases: [&'static str; 7],
}

impl UpdateMessages {
    /// Enumerate every owned label for complete registry coverage checks.
    pub const fn labels(&self) -> [&'static str; 16] {
        let p = self.phases;
        [
            self.ready,
            self.failed,
            self.restart,
            self.restart_after_workflow,
            self.dismiss,
            self.whats_new,
            self.applying,
            self.updated,
            self.previous_running,
            p[0],
            p[1],
            p[2],
            p[3],
            p[4],
            p[5],
            p[6],
        ]
    }

    /// The report's name for the step an attempt reached.
    pub const fn phase(&self, phase: AttemptPhase) -> &'static str {
        self.phases[match phase {
            AttemptPhase::Opening => 0,
            AttemptPhase::Checking => 1,
            AttemptPhase::CheckingUpdater => 2,
            AttemptPhase::Downloading => 3,
            AttemptPhase::Preparing => 4,
            AttemptPhase::PreparingUpdater => 5,
            AttemptPhase::Applying => 6,
        }]
    }

    /// `template` with its [`VERSION`] placeholder replaced by `version`.
    pub fn with_version(template: &str, version: &str) -> String {
        template.replacen(VERSION, version, 1)
    }
}

/// Canonical English update words.
pub static EN: UpdateMessages = UpdateMessages {
    ready: "Update ready",
    failed: "Couldn't update. Show the report",
    restart: "Restart to apply",
    restart_after_workflow: "Restart after this finishes",
    dismiss: "Dismiss",
    whats_new: "What's new",
    applying: "Updating to version {version}…",
    updated: "Updated to version {version}",
    previous_running: "The previous version is running.",
    phases: [
        "Getting ready",
        "Checking for updates",
        "Checking the updater",
        "Downloading the update",
        "Preparing the update",
        "Preparing the updater",
        "Starting the new version",
    ],
};

/// Simplified Chinese update words.
pub static ZH_HANS: UpdateMessages = UpdateMessages {
    ready: "更新已就绪",
    failed: "无法更新。查看报告",
    restart: "重新启动以应用",
    restart_after_workflow: "完成后重新启动",
    dismiss: "关闭",
    whats_new: "新功能",
    applying: "正在更新到版本 {version}…",
    updated: "已更新到版本 {version}",
    previous_running: "正在运行之前的版本。",
    phases: [
        "正在准备",
        "正在检查更新",
        "正在检查更新程序",
        "正在下载更新",
        "正在准备更新",
        "正在准备更新程序",
        "正在启动新版本",
    ],
};

/// Japanese update words.
pub static JA: UpdateMessages = UpdateMessages {
    ready: "更新の準備ができました",
    failed: "更新できませんでした。レポートを表示",
    restart: "再起動して適用",
    restart_after_workflow: "完了後に再起動",
    dismiss: "閉じる",
    whats_new: "新機能",
    applying: "バージョン {version} に更新しています…",
    updated: "バージョン {version} に更新しました",
    previous_running: "以前のバージョンで動作しています。",
    phases: [
        "準備中",
        "更新を確認中",
        "アップデーターを確認中",
        "更新をダウンロード中",
        "更新を準備中",
        "アップデーターを準備中",
        "新しいバージョンを起動中",
    ],
};

/// Korean update words.
pub static KO: UpdateMessages = UpdateMessages {
    ready: "업데이트 준비됨",
    failed: "업데이트할 수 없습니다. 보고서 보기",
    restart: "다시 시작하여 적용",
    restart_after_workflow: "완료 후 다시 시작",
    dismiss: "닫기",
    whats_new: "새로운 기능",
    applying: "버전 {version}(으)로 업데이트하는 중…",
    updated: "버전 {version}(으)로 업데이트됨",
    previous_running: "이전 버전이 실행 중입니다.",
    phases: [
        "준비 중",
        "업데이트 확인 중",
        "업데이트 프로그램 확인 중",
        "업데이트 다운로드 중",
        "업데이트 준비 중",
        "업데이트 프로그램 준비 중",
        "새 버전 시작 중",
    ],
};

/// Spanish update words.
pub static ES: UpdateMessages = UpdateMessages {
    ready: "Actualización lista",
    failed: "No se pudo actualizar. Ver el informe",
    restart: "Reiniciar para aplicar",
    restart_after_workflow: "Reiniciar al terminar",
    dismiss: "Descartar",
    whats_new: "Novedades",
    applying: "Actualizando a la versión {version}…",
    updated: "Actualizado a la versión {version}",
    previous_running: "Se está usando la versión anterior.",
    phases: [
        "Preparando",
        "Buscando actualizaciones",
        "Comprobando el actualizador",
        "Descargando la actualización",
        "Preparando la actualización",
        "Preparando el actualizador",
        "Iniciando la nueva versión",
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_templates_name_the_version_exactly_once_in_every_locale() {
        for locale in crate::localization::LOCALES {
            for template in [locale.updates.applying, locale.updates.updated] {
                assert_eq!(template.matches(VERSION).count(), 1, "{}: {template}", locale.id);
                let filled = UpdateMessages::with_version(template, "2.4.1");
                assert!(filled.contains("2.4.1") && !filled.contains(VERSION));
            }
            for label in locale.updates.labels() {
                assert!(!label.trim().is_empty(), "{}", locale.id);
            }
        }
    }

    #[test]
    fn every_attempt_step_has_its_own_name() {
        let phases = [
            AttemptPhase::Opening,
            AttemptPhase::Checking,
            AttemptPhase::CheckingUpdater,
            AttemptPhase::Downloading,
            AttemptPhase::Preparing,
            AttemptPhase::PreparingUpdater,
            AttemptPhase::Applying,
        ];
        let names: std::collections::BTreeSet<_> =
            phases.into_iter().map(|phase| EN.phase(phase)).collect();
        assert_eq!(names.len(), phases.len());
        assert_eq!(EN.phase(AttemptPhase::Downloading), "Downloading the update");
    }
}
