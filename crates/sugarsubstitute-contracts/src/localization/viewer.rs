// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own standalone native canvas status copy through the supported-locale registry.

/// Native viewer status; image filenames and authored document text stay with their owners.
pub struct ViewerMessages {
    /// Native OS window title.
    pub title: &'static str,
    /// Empty viewer and available native open shortcut.
    pub empty: &'static str,
    /// Current original image is being imported.
    pub opening: &'static str,
    /// Actionable open/render failure without disclosing sensitive source paths.
    pub failed: &'static str,
}
impl ViewerMessages {
    /// Compose current physical zoom status using this catalog's native title and percentage style.
    pub fn viewing(&self, zoom: f64) -> String {
        format!("{} · {:.1}%", self.title, zoom * 100.0)
    }
    /// Enumerate complete native viewer copy for registry coverage qualification.
    pub const fn labels(&self) -> [&'static str; 4] {
        [self.title, self.empty, self.opening, self.failed]
    }
}
/// Canonical English viewer copy.
pub static EN: ViewerMessages = ViewerMessages {
    title: "SugarSubstitute Canvas",
    empty: "Press Ctrl+O to open an image",
    opening: "Opening image…",
    failed: "Unable to open or render the image. Press Ctrl+O to open another image.",
};
/// Simplified Chinese viewer copy.
pub static ZH_HANS: ViewerMessages = ViewerMessages {
    title: "SugarSubstitute 画布",
    empty: "按 Ctrl+O 打开图像",
    opening: "正在打开图像…",
    failed: "无法打开或渲染图像。按 Ctrl+O 打开另一张图像。",
};
/// Japanese viewer copy.
pub static JA: ViewerMessages = ViewerMessages {
    title: "SugarSubstitute キャンバス",
    empty: "Ctrl+O で画像を開きます",
    opening: "画像を開いています…",
    failed: "画像を開くか描画することができませんでした。Ctrl+O で別の画像を開いてください。",
};
/// Korean viewer copy.
pub static KO: ViewerMessages = ViewerMessages {
    title: "SugarSubstitute 캔버스",
    empty: "Ctrl+O를 눌러 이미지를 여세요",
    opening: "이미지를 여는 중…",
    failed: "이미지를 열거나 렌더링할 수 없습니다. Ctrl+O를 눌러 다른 이미지를 여세요.",
};
/// Spanish viewer copy.
pub static ES: ViewerMessages = ViewerMessages {
    title: "Lienzo de SugarSubstitute",
    empty: "Pulsa Ctrl+O para abrir una imagen",
    opening: "Abriendo imagen…",
    failed: "No se pudo abrir o dibujar la imagen. Pulsa Ctrl+O para abrir otra imagen.",
};
