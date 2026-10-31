// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check the exact public presentation contract and full-window browser geometry.
use super::*;

#[test]
fn public_desktop_frame_receipt_rejects_unsupported_or_foreign_presentation() {
    for (json, dark, accepted) in [
        (
            r#"{"schemaVersion":1,"captionWidth":138,"titlebarHeight":34,"darkTheme":true,"mounted":true}"#,
            true,
            true,
        ),
        (
            r#"{"schemaVersion":1,"captionWidth":138,"titlebarHeight":34,"darkTheme":false,"mounted":false}"#,
            false,
            true,
        ),
        (
            r#"{"schemaVersion":2,"captionWidth":138,"titlebarHeight":34,"darkTheme":true,"mounted":true}"#,
            true,
            false,
        ),
        (
            r#"{"schemaVersion":1,"captionWidth":144,"titlebarHeight":34,"darkTheme":true,"mounted":true}"#,
            true,
            false,
        ),
        (
            r#"{"schemaVersion":1,"captionWidth":138,"titlebarHeight":40,"darkTheme":true,"mounted":true}"#,
            true,
            false,
        ),
        (
            r#"{"schemaVersion":1,"captionWidth":138,"titlebarHeight":34,"darkTheme":false,"mounted":true}"#,
            true,
            false,
        ),
        (
            r#"{"schemaVersion":1,"captionWidth":138,"titlebarHeight":34,"darkTheme":true,"mounted":true,"nativePrivilege":true}"#,
            true,
            false,
        ),
        (
            r#"{"schemaVersion":1,"captionWidth":138,"titlebarHeight":34,"darkTheme":true}"#,
            true,
            false,
        ),
    ] {
        let receipt = serde_json::from_str::<Receipt>(json);
        let result = receipt.as_ref().is_ok_and(|value| value.validate(dark).is_ok());
        assert_eq!(result, accepted, "{json}");
    }
}

#[test]
fn configuration_request_is_closed_and_does_not_accept_ready_or_incomplete_receipts() {
    assert!(Needed::decode(r#"{"configure_desktop_frame":1}"#));
    for value in [
        r#"{"configure_desktop_frame":2}"#,
        r#"{"incomplete":1}"#,
        r#"{"configure_desktop_frame":1,"ready":true}"#,
        r#"{"configure_desktop_frame":true}"#,
    ] {
        assert!(!Needed::decode(value));
    }
}

#[test]
fn browser_uses_every_physical_client_pixel_at_any_scale_without_caption_inset() {
    for (scale, width, height) in [(1.0, 304, 240), (1.25, 1001, 667), (1.75, 2241, 1261)] {
        let bounds = browser_bounds(width, height);
        let position = bounds.position.to_physical::<i32>(scale);
        let size = bounds.size.to_physical::<u32>(scale);
        assert_eq!((position.x, position.y), (0, 0));
        assert_eq!((size.width, size.height), (width, height), "no rounding at {scale}");
    }
    let zero = browser_bounds(0, 0).size.to_physical::<u32>(1.0);
    assert_eq!((zero.width, zero.height), (1, 1));
}
