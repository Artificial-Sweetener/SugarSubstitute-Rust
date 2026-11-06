// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare every shade against Qt-generated vectors shared byte-for-byte with SugarCubes.
use super::*;

/// Parse a fixture channel without allowing malformed vectors to masquerade as black.
fn rgb(hex: &str) -> Result<[u8; 4], Box<dyn std::error::Error>> {
    Ok([
        u8::from_str_radix(&hex[0..2], 16)?,
        u8::from_str_radix(&hex[2..4], 16)?,
        u8::from_str_radix(&hex[4..6], 16)?,
        255,
    ])
}

/// Cover hue sectors, achromatic seeds, saturation/value clamps and byte-rounding boundaries.
#[test]
fn seven_variants_match_shared_qfluentwidgets_vectors() -> Result<(), Box<dyn std::error::Error>> {
    let mut count = 0;
    for line in include_str!("vectors.txt").lines().filter(|line| !line.starts_with('#')) {
        let columns = line.split_whitespace().collect::<Vec<_>>();
        assert_eq!(columns.len(), 9);
        let [r, g, b, _] = rgb(columns[0])?;
        let p = AccentPalette::system(AccentColor::new(r, g, b)).variants(columns[1] == "dark");
        for (actual, expected) in
            [p.primary, p.dark1, p.dark2, p.dark3, p.light1, p.light2, p.light3]
                .into_iter()
                .zip(&columns[2..])
        {
            assert_eq!(actual, rgb(expected)?, "{line}");
        }
        let hover = if p.dark { p.dark1 } else { p.light1 };
        assert_eq!(p.button(ControlState::Rest), p.primary);
        assert_eq!(p.button(ControlState::Hover), hover);
        assert_eq!(p.button(ControlState::Pressed), if p.dark { p.dark2 } else { p.light3 });
        assert_eq!(p.checkbox(ControlState::Hover), hover);
        assert_eq!(p.checkbox(ControlState::Pressed), if p.dark { p.dark2 } else { p.light2 });
        assert_eq!(p.toggle(ControlState::Hover), p.light1);
        assert_eq!(p.toggle(ControlState::Pressed), p.light2);
        assert_eq!(p.link(ControlState::Pressed), if p.dark { p.dark1 } else { p.primary });
        assert_eq!(p.selection(), if p.dark { p.primary } else { p.light1 });
        assert_eq!(
            p.foreground(ControlState::Rest),
            if p.dark { [0, 0, 0, 255] } else { [255; 4] }
        );
        assert_eq!(
            p.foreground(ControlState::Pressed),
            if p.dark { [0, 0, 0, 161] } else { [255, 255, 255, 161] }
        );
        count += 1;
    }
    assert_eq!(count, 60);
    Ok(())
}

/// Absence changes the seed policy, never the derivation algorithm.
#[test]
fn missing_accent_uses_the_canonical_product_seed() {
    assert_eq!(AccentPalette::resolve(None), AccentPalette::PRODUCT);
    assert_eq!(AccentPalette::PRODUCT.progress(true), [255, 68, 132, 255]);
    assert_eq!(AccentPalette::PRODUCT.progress(false), [233, 30, 99, 255]);
}

/// Regression: a dark primary and its hover differ from the configured system RGB.
#[test]
fn dark_system_accent_matches_qfluentwidgets() {
    let p = AccentPalette::system(AccentColor::new(0, 120, 212));
    assert_eq!(p.progress(true), [41, 162, 255, 255]);
    assert_eq!(p.variants(true).button(ControlState::Hover), [37, 146, 230, 255]);
}
