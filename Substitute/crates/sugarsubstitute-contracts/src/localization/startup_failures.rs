// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Localized reasons a start failed, shown on the splash while it tries again by itself.

use crate::startup::StartupFailureCode;

/// One line per bounded startup failure reason.
#[non_exhaustive]
pub struct StartupFailureMessages {
    /// No ComfyUI answered at its address.
    pub unreachable: &'static str,
    /// ComfyUI serves another frontend version than the installed one.
    pub frontend_mismatch: &'static str,
    /// ComfyUI does not serve SugarCubes.
    pub sugarcubes_missing: &'static str,
    /// The started ComfyUI ended before it was ready.
    pub exited: &'static str,
    /// The start did not finish in its allowed time.
    pub timed_out: &'static str,
    /// Any other failed start.
    pub other: &'static str,
    /// The status line while the start is retried: `{reason}` is replaced by the reason.
    pub retrying: &'static str,
    /// Heading of the dialog for a failure retrying by itself cannot fix.
    pub dialog_title: &'static str,
    /// The dialog's button that starts again.
    pub dialog_retry: &'static str,
    /// The dialog's button that closes the application.
    pub dialog_close: &'static str,
    /// What to do when ComfyUI serves another frontend version.
    pub frontend_mismatch_help: &'static str,
    /// What to do when ComfyUI does not serve SugarCubes.
    pub sugarcubes_missing_help: &'static str,
    /// What to do when ComfyUI's SugarCubes is older than this application needs.
    pub sugarcubes_outdated_help: &'static str,
    /// What to do when several tries in a row could not connect.
    pub retries_exhausted_help: &'static str,
}

impl StartupFailureMessages {
    /// The line for `code`.
    pub const fn reason(&self, code: StartupFailureCode) -> &'static str {
        match code {
            StartupFailureCode::BackendUnreachable => self.unreachable,
            StartupFailureCode::FrontendMismatch => self.frontend_mismatch,
            StartupFailureCode::SugarCubesMissing => self.sugarcubes_missing,
            StartupFailureCode::ProcessExited => self.exited,
            StartupFailureCode::TimedOut => self.timed_out,
            StartupFailureCode::ConnectionPreparation
            | StartupFailureCode::ConnectionUnavailable
            | StartupFailureCode::Presentation => self.other,
        }
    }

    /// The status line for `code` while the start is retried by itself.
    pub fn retrying(&self, code: StartupFailureCode) -> String {
        self.retrying.replace("{reason}", self.reason(code))
    }

    /// Every line, for coverage checks.
    pub const fn labels(&self) -> [&'static str; 14] {
        [
            self.retries_exhausted_help,
            self.dialog_title,
            self.dialog_retry,
            self.dialog_close,
            self.frontend_mismatch_help,
            self.sugarcubes_missing_help,
            self.sugarcubes_outdated_help,
            self.unreachable,
            self.frontend_mismatch,
            self.sugarcubes_missing,
            self.exited,
            self.timed_out,
            self.other,
            self.retrying,
        ]
    }
}

/// English.
pub static EN: StartupFailureMessages = StartupFailureMessages {
    unreachable: "ComfyUI is not answering",
    frontend_mismatch: "ComfyUI runs a different frontend version",
    sugarcubes_missing: "ComfyUI is not serving SugarCubes",
    exited: "ComfyUI stopped while starting",
    timed_out: "ComfyUI took too long to start",
    other: "Couldn't connect to ComfyUI",
    retrying: "{reason}. Trying again…",
    dialog_title: "SugarSubstitute couldn't connect to ComfyUI",
    dialog_retry: "Retry",
    dialog_close: "Close SugarSubstitute",
    frontend_mismatch_help: "ComfyUI is running a different version of its frontend than SugarSubstitute uses. Update ComfyUI's frontend (comfyui-frontend-package) to the version SugarSubstitute expects and restart ComfyUI, then choose Retry.",
    sugarcubes_missing_help: "The ComfyUI that SugarSubstitute connects to is not serving SugarCubes. Install SugarCubes in that ComfyUI's custom_nodes folder and restart ComfyUI, then choose Retry.",
    sugarcubes_outdated_help: "The SugarCubes installed in ComfyUI is older than this SugarSubstitute needs. Update SugarCubes and restart ComfyUI, then choose Retry.",
    retries_exhausted_help: "SugarSubstitute tried to connect to ComfyUI several times and could not. Check that ComfyUI is running and has finished starting, then choose Retry.",
};

/// Simplified Chinese.
pub static ZH_HANS: StartupFailureMessages = StartupFailureMessages {
    unreachable: "ComfyUI 没有响应",
    frontend_mismatch: "ComfyUI 运行的是不同版本的前端",
    sugarcubes_missing: "ComfyUI 没有提供 SugarCubes",
    exited: "ComfyUI 在启动时停止了",
    timed_out: "ComfyUI 启动时间过长",
    other: "无法连接到 ComfyUI",
    retrying: "{reason}，正在重试…",
    dialog_title: "SugarSubstitute 无法连接到 ComfyUI",
    dialog_retry: "重试",
    dialog_close: "关闭 SugarSubstitute",
    frontend_mismatch_help: "ComfyUI 运行的前端版本与 SugarSubstitute 使用的不同。请将 ComfyUI 的前端（comfyui-frontend-package）更新到 SugarSubstitute 需要的版本并重新启动 ComfyUI，然后选择“重试”。",
    sugarcubes_missing_help: "SugarSubstitute 连接的 ComfyUI 没有提供 SugarCubes。请将 SugarCubes 安装到该 ComfyUI 的 custom_nodes 文件夹并重新启动 ComfyUI，然后选择“重试”。",
    sugarcubes_outdated_help: "ComfyUI 中安装的 SugarCubes 版本低于此 SugarSubstitute 的要求。请更新 SugarCubes 并重新启动 ComfyUI，然后选择“重试”。",
    retries_exhausted_help: "SugarSubstitute 多次尝试连接 ComfyUI 均未成功。请确认 ComfyUI 正在运行并已完成启动，然后选择“重试”。",
};

/// Japanese.
pub static JA: StartupFailureMessages = StartupFailureMessages {
    unreachable: "ComfyUI が応答していません",
    frontend_mismatch: "ComfyUI のフロントエンドのバージョンが異なります",
    sugarcubes_missing: "ComfyUI が SugarCubes を提供していません",
    exited: "ComfyUI が起動中に停止しました",
    timed_out: "ComfyUI の起動に時間がかかりすぎました",
    other: "ComfyUI に接続できませんでした",
    retrying: "{reason}。再試行しています…",
    dialog_title: "SugarSubstitute が ComfyUI に接続できませんでした",
    dialog_retry: "再試行",
    dialog_close: "SugarSubstitute を閉じる",
    frontend_mismatch_help: "ComfyUI のフロントエンドのバージョンが SugarSubstitute の使うものと異なります。ComfyUI のフロントエンド（comfyui-frontend-package）を SugarSubstitute が必要とするバージョンに更新して ComfyUI を再起動してから、「再試行」を選んでください。",
    sugarcubes_missing_help: "SugarSubstitute が接続する ComfyUI が SugarCubes を提供していません。その ComfyUI の custom_nodes フォルダーに SugarCubes をインストールして ComfyUI を再起動してから、「再試行」を選んでください。",
    sugarcubes_outdated_help: "ComfyUI にインストールされている SugarCubes が、この SugarSubstitute の必要とするバージョンより古いです。SugarCubes を更新して ComfyUI を再起動してから、「再試行」を選んでください。",
    retries_exhausted_help: "SugarSubstitute は ComfyUI への接続を何度か試みましたが、接続できませんでした。ComfyUI が実行中で起動が完了していることを確認してから、「再試行」を選んでください。",
};

/// Korean.
pub static KO: StartupFailureMessages = StartupFailureMessages {
    unreachable: "ComfyUI가 응답하지 않습니다",
    frontend_mismatch: "ComfyUI의 프런트엔드 버전이 다릅니다",
    sugarcubes_missing: "ComfyUI가 SugarCubes를 제공하지 않습니다",
    exited: "ComfyUI가 시작 중에 중지되었습니다",
    timed_out: "ComfyUI 시작에 시간이 너무 오래 걸렸습니다",
    other: "ComfyUI에 연결할 수 없습니다",
    retrying: "{reason}. 다시 시도하는 중…",
    dialog_title: "SugarSubstitute가 ComfyUI에 연결할 수 없습니다",
    dialog_retry: "다시 시도",
    dialog_close: "SugarSubstitute 닫기",
    frontend_mismatch_help: "ComfyUI의 프런트엔드 버전이 SugarSubstitute가 사용하는 버전과 다릅니다. ComfyUI의 프런트엔드(comfyui-frontend-package)를 SugarSubstitute가 필요로 하는 버전으로 업데이트하고 ComfyUI를 다시 시작한 다음 '다시 시도'를 선택하세요.",
    sugarcubes_missing_help: "SugarSubstitute가 연결하는 ComfyUI가 SugarCubes를 제공하지 않습니다. 해당 ComfyUI의 custom_nodes 폴더에 SugarCubes를 설치하고 ComfyUI를 다시 시작한 다음 '다시 시도'를 선택하세요.",
    sugarcubes_outdated_help: "ComfyUI에 설치된 SugarCubes가 이 SugarSubstitute에 필요한 버전보다 오래되었습니다. SugarCubes를 업데이트하고 ComfyUI를 다시 시작한 다음 '다시 시도'를 선택하세요.",
    retries_exhausted_help: "SugarSubstitute가 ComfyUI에 여러 번 연결을 시도했지만 연결할 수 없었습니다. ComfyUI가 실행 중이고 시작이 끝났는지 확인한 다음 '다시 시도'를 선택하세요.",
};

/// Spanish.
pub static ES: StartupFailureMessages = StartupFailureMessages {
    unreachable: "ComfyUI no responde",
    frontend_mismatch: "ComfyUI usa otra versión del frontend",
    sugarcubes_missing: "ComfyUI no ofrece SugarCubes",
    exited: "ComfyUI se detuvo al iniciarse",
    timed_out: "ComfyUI tardó demasiado en iniciarse",
    other: "No se pudo conectar con ComfyUI",
    retrying: "{reason}. Reintentando…",
    dialog_title: "SugarSubstitute no pudo conectarse con ComfyUI",
    dialog_retry: "Reintentar",
    dialog_close: "Cerrar SugarSubstitute",
    frontend_mismatch_help: "ComfyUI usa una versión de su frontend distinta de la que usa SugarSubstitute. Actualiza el frontend de ComfyUI (comfyui-frontend-package) a la versión que espera SugarSubstitute y reinicia ComfyUI; después elige Reintentar.",
    sugarcubes_missing_help: "El ComfyUI al que se conecta SugarSubstitute no ofrece SugarCubes. Instala SugarCubes en la carpeta custom_nodes de ese ComfyUI y reinicia ComfyUI; después elige Reintentar.",
    sugarcubes_outdated_help: "El SugarCubes instalado en ComfyUI es más antiguo de lo que necesita este SugarSubstitute. Actualiza SugarCubes y reinicia ComfyUI; después elige Reintentar.",
    retries_exhausted_help: "SugarSubstitute intentó conectarse con ComfyUI varias veces y no lo consiguió. Comprueba que ComfyUI se esté ejecutando y haya terminado de iniciarse; después elige Reintentar.",
};
