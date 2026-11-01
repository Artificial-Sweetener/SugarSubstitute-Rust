// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Staged names keep the output's own leaf and can never leave their staging entry.

use super::*;

#[test]
fn staged_names_keep_the_original_leaf() {
    assert_eq!(staged_name("ComfyUI_00042_.png"), "ComfyUI_00042_.png");
    assert_eq!(staged_name("渲染 输出.jpg"), "渲染 输出.jpg");
}

#[test]
fn traversal_and_reserved_names_become_one_safe_leaf() {
    for (original, staged) in [
        ("../../escape.png", "escape.png"),
        ("..\\..\\Windows\\escape.png", "escape.png"),
        ("C:\\Users\\image.png", "image.png"),
        ("C:image.png", "C_image.png"),
        ("a<b>|?*\".png", "a_b_____.png"),
        ("line\nbreak.png", "line_break.png"),
        ("trailing. .", "trailing"),
        ("CON.png", "_CON.png"),
        ("com1", "_com1"),
        ("..", "output"),
        ("", "output"),
        ("folder/", "output"),
    ] {
        assert_eq!(staged_name(original), staged, "{original:?}");
    }
    let long = staged_name(&"a".repeat(400));
    assert_eq!(long.encode_utf16().count(), MAX_NAME_UNITS);
}
