// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the words of the SugarCubes update that runs during the splash: the one line a start
//! shows about it and the lines of its update report.

/// Words for the SugarCubes update during start-up.
///
/// Lines that name a version contain [`super::updates::VERSION`]; fill them with
/// [`super::updates::UpdateMessages::with_version`].
#[non_exhaustive]
pub struct PackUpdateMessages {
    /// Splash line while a SugarCubes update holds the start.
    pub updating: &'static str,
    /// One line: SugarCubes was updated during this start; contains the version.
    pub updated: &'static str,
    /// One line: neither release source answered, so the installed version runs.
    pub could_not_check: &'static str,
    /// One line: the update failed, so the installed version runs.
    pub could_not_update: &'static str,
    /// One line: the last update did not start, so the version before it runs again.
    pub previous_running: &'static str,
    /// One line: a newer version exists for a ComfyUI this app does not start; contains it.
    pub available: &'static str,
    /// Report: SugarCubes is up to date.
    pub up_to_date: &'static str,
    /// Report: the Comfy Registry did not answer.
    pub registry_unchecked: &'static str,
    /// Report: GitHub did not answer.
    pub github_unchecked: &'static str,
    /// Report: the folder is a git checkout and is left alone.
    pub kept_git: &'static str,
    /// Report: files of the installed version were changed and are left alone.
    pub kept_edited: &'static str,
    /// Report: the folder is a link and is left alone.
    pub kept_linked: &'static str,
    /// Report: the folder was not installed from the registry and is left alone.
    pub kept_unregistered: &'static str,
    /// Report: SugarCubes' own updater was changing the folder, so nothing was done.
    pub busy: &'static str,
    /// Report step names: checking, downloading, installing.
    pub steps: [&'static str; 3],
}

impl PackUpdateMessages {
    /// Enumerate every owned label for complete registry coverage checks.
    pub const fn labels(&self) -> [&'static str; 17] {
        let s = self.steps;
        [
            self.updating,
            self.updated,
            self.could_not_check,
            self.could_not_update,
            self.previous_running,
            self.available,
            self.up_to_date,
            self.registry_unchecked,
            self.github_unchecked,
            self.kept_git,
            self.kept_edited,
            self.kept_linked,
            self.kept_unregistered,
            self.busy,
            s[0],
            s[1],
            s[2],
        ]
    }
}

/// Canonical English words.
pub static EN: PackUpdateMessages = PackUpdateMessages {
    updating: "Updating SugarCubes…",
    updated: "SugarCubes updated to version {version}",
    could_not_check: "Couldn't check for a SugarCubes update.",
    could_not_update: "Couldn't update SugarCubes. The installed version is running.",
    previous_running: "The previous SugarCubes version is running.",
    available: "SugarCubes version {version} is available.",
    up_to_date: "SugarCubes is up to date.",
    registry_unchecked: "Couldn't check the Comfy Registry.",
    github_unchecked: "Couldn't check GitHub.",
    kept_git: "SugarCubes is a git checkout, so it isn't updated here.",
    kept_edited: "Some SugarCubes files were changed, so it isn't updated here.",
    kept_linked: "The SugarCubes folder is a link, so it isn't updated here.",
    kept_unregistered: "SugarCubes wasn't installed from the registry, so it isn't updated here.",
    busy: "SugarCubes was updating itself, so it was left as it is.",
    steps: ["Checking for a SugarCubes update", "Downloading SugarCubes", "Installing SugarCubes"],
};

/// Simplified Chinese words.
pub static ZH_HANS: PackUpdateMessages = PackUpdateMessages {
    updating: "正在更新 SugarCubes…",
    updated: "SugarCubes 已更新到版本 {version}",
    could_not_check: "无法检查 SugarCubes 更新。",
    could_not_update: "无法更新 SugarCubes。正在运行已安装的版本。",
    previous_running: "正在运行之前的 SugarCubes 版本。",
    available: "SugarCubes 版本 {version} 可用。",
    up_to_date: "SugarCubes 已是最新版本。",
    registry_unchecked: "无法检查 Comfy Registry。",
    github_unchecked: "无法检查 GitHub。",
    kept_git: "SugarCubes 是 git 签出，因此不在此处更新。",
    kept_edited: "部分 SugarCubes 文件已被修改，因此不在此处更新。",
    kept_linked: "SugarCubes 文件夹是一个链接，因此不在此处更新。",
    kept_unregistered: "SugarCubes 不是从 Registry 安装的，因此不在此处更新。",
    busy: "SugarCubes 正在自行更新，因此保持原样。",
    steps: ["正在检查 SugarCubes 更新", "正在下载 SugarCubes", "正在安装 SugarCubes"],
};

/// Japanese words.
pub static JA: PackUpdateMessages = PackUpdateMessages {
    updating: "SugarCubes を更新しています…",
    updated: "SugarCubes をバージョン {version} に更新しました",
    could_not_check: "SugarCubes の更新を確認できませんでした。",
    could_not_update: "SugarCubes を更新できませんでした。インストール済みのバージョンで動作しています。",
    previous_running: "以前の SugarCubes バージョンで動作しています。",
    available: "SugarCubes バージョン {version} を利用できます。",
    up_to_date: "SugarCubes は最新です。",
    registry_unchecked: "Comfy Registry を確認できませんでした。",
    github_unchecked: "GitHub を確認できませんでした。",
    kept_git: "SugarCubes は git のチェックアウトなので、ここでは更新しません。",
    kept_edited: "SugarCubes の一部のファイルが変更されているので、ここでは更新しません。",
    kept_linked: "SugarCubes のフォルダーはリンクなので、ここでは更新しません。",
    kept_unregistered: "SugarCubes は Registry からインストールされていないので、ここでは更新しません。",
    busy: "SugarCubes が自身を更新中だったため、そのままにしました。",
    steps: [
        "SugarCubes の更新を確認しています",
        "SugarCubes をダウンロードしています",
        "SugarCubes をインストールしています",
    ],
};

/// Korean words.
pub static KO: PackUpdateMessages = PackUpdateMessages {
    updating: "SugarCubes 업데이트 중…",
    updated: "SugarCubes를 버전 {version}(으)로 업데이트했습니다",
    could_not_check: "SugarCubes 업데이트를 확인할 수 없습니다.",
    could_not_update: "SugarCubes를 업데이트할 수 없습니다. 설치된 버전이 실행 중입니다.",
    previous_running: "이전 SugarCubes 버전이 실행 중입니다.",
    available: "SugarCubes 버전 {version}을(를) 사용할 수 있습니다.",
    up_to_date: "SugarCubes가 최신 버전입니다.",
    registry_unchecked: "Comfy Registry를 확인할 수 없습니다.",
    github_unchecked: "GitHub를 확인할 수 없습니다.",
    kept_git: "SugarCubes가 git 체크아웃이므로 여기서 업데이트하지 않습니다.",
    kept_edited: "일부 SugarCubes 파일이 변경되어 여기서 업데이트하지 않습니다.",
    kept_linked: "SugarCubes 폴더가 링크이므로 여기서 업데이트하지 않습니다.",
    kept_unregistered: "SugarCubes가 Registry에서 설치되지 않았으므로 여기서 업데이트하지 않습니다.",
    busy: "SugarCubes가 스스로 업데이트하는 중이어서 그대로 두었습니다.",
    steps: ["SugarCubes 업데이트 확인 중", "SugarCubes 다운로드 중", "SugarCubes 설치 중"],
};

/// Spanish words.
pub static ES: PackUpdateMessages = PackUpdateMessages {
    updating: "Actualizando SugarCubes…",
    updated: "SugarCubes se actualizó a la versión {version}",
    could_not_check: "No se pudo comprobar si hay una actualización de SugarCubes.",
    could_not_update: "No se pudo actualizar SugarCubes. Se está usando la versión instalada.",
    previous_running: "Se está usando la versión anterior de SugarCubes.",
    available: "La versión {version} de SugarCubes está disponible.",
    up_to_date: "SugarCubes está actualizado.",
    registry_unchecked: "No se pudo comprobar el Comfy Registry.",
    github_unchecked: "No se pudo comprobar GitHub.",
    kept_git: "SugarCubes es una copia de git, así que no se actualiza aquí.",
    kept_edited: "Se cambiaron algunos archivos de SugarCubes, así que no se actualiza aquí.",
    kept_linked: "La carpeta de SugarCubes es un enlace, así que no se actualiza aquí.",
    kept_unregistered: "SugarCubes no se instaló desde el Registry, así que no se actualiza aquí.",
    busy: "SugarCubes se estaba actualizando, así que se dejó como estaba.",
    steps: [
        "Buscando una actualización de SugarCubes",
        "Descargando SugarCubes",
        "Instalando SugarCubes",
    ],
};
