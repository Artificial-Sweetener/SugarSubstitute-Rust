// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify native provider contracts and real UIA clients within an owned application lifetime.
use super::qualification::qualify_client;
use sugarsubstitute_ui::windows::text_style::TextStyle;

#[test]
fn native_windowless_patterns_keep_unicode_and_revoke_escaped_ranges()
-> Result<(), Box<dyn std::error::Error>> {
    crate::windows_test_desktop::run(
        concat!(
            module_path!(),
            "::native_windowless_patterns_keep_unicode_and_revoke_escaped_ranges"
        ),
        || qualify_client(TextStyle::Transcript),
    )
}

#[test]
fn native_editable_patterns_keep_unicode_and_revoke_escaped_ranges()
-> Result<(), Box<dyn std::error::Error>> {
    crate::windows_test_desktop::run(
        concat!(
            module_path!(),
            "::native_editable_patterns_keep_unicode_and_revoke_escaped_ranges"
        ),
        || qualify_client(TextStyle::Field),
    )
}
