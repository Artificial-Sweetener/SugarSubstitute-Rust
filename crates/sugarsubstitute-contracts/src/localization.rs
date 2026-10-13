// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own SugarSubstitute's product copy and its sole supported-locale inventory.

/// Localized native incident reporting copy.
pub mod incident;
/// Localized native main-window controls.
pub mod window;

/// Bind one supported locale to every user-facing splash message.
pub struct Messages {
    /// Locale identifier accepted by the executable.
    pub id: &'static str,
    /// Initial status and native window title.
    pub loading: &'static str,
    /// Accessible intent for the collapsed output button.
    pub show_output: &'static str,
    /// Accessible intent for the expanded output button.
    pub hide_output: &'static str,
    /// Accessible intent for the close button.
    pub cancel: &'static str,
    /// Complete native report surface copy.
    pub report: &'static incident::ReportMessages,
    /// Accessible names for the native main-window caption controls.
    pub window: &'static window::WindowMessages,
}

/// Keep the sole locale inventory together with complete owned-surface coverage.
pub static LOCALES: &[Messages] = &[
    Messages {
        id: "en",
        loading: "Loading...",
        show_output: "Show Comfy output",
        hide_output: "Hide Comfy output",
        cancel: "Cancel loading",
        report: &incident::EN,
        window: &window::EN,
    },
    Messages {
        id: "zh-Hans",
        loading: "正在加载...",
        show_output: "显示 Comfy 输出",
        hide_output: "隐藏 Comfy 输出",
        cancel: "取消加载",
        report: &incident::ZH_HANS,
        window: &window::ZH_HANS,
    },
    Messages {
        id: "ja",
        loading: "読み込み中...",
        show_output: "Comfy の出力を表示",
        hide_output: "Comfy の出力を非表示",
        cancel: "読み込みをキャンセル",
        report: &incident::JA,
        window: &window::JA,
    },
    Messages {
        id: "ko",
        loading: "불러오는 중...",
        show_output: "Comfy 출력 표시",
        hide_output: "Comfy 출력 숨기기",
        cancel: "불러오기 취소",
        report: &incident::KO,
        window: &window::KO,
    },
    Messages {
        id: "es",
        loading: "Cargando...",
        show_output: "Mostrar la salida de Comfy",
        hide_output: "Ocultar la salida de Comfy",
        cancel: "Cancelar la carga",
        report: &incident::ES,
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
                [locale.loading, locale.show_output, locale.hide_output, locale.cancel]
                    .into_iter()
                    .chain(locale.report.labels())
                    .chain(locale.window.labels())
                    .all(|text| !text.is_empty() && !text.contains('\0'))
            );
        }
        assert_eq!(messages("unrecognized").id, "en");
    }
}
