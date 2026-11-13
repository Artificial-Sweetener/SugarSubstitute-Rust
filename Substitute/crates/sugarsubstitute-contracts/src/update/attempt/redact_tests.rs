// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Redaction of paths, addresses and credentials, checked as a fixed point on every example.
use super::*;

/// Sanitize `text`, check that sanitizing again changes nothing, and return the result.
fn redacted(text: &str) -> String {
    let once = sanitize(text);
    assert_eq!(sanitize(&once), once, "not a fixed point for {text:?}");
    once
}

#[test]
fn local_paths_of_every_form_become_one_placeholder() {
    let cases = [
        (
            r#"Cannot open "C:\Users\someone\SugarSubstitute\user\x.json" now"#,
            "Cannot open <path> now",
        ),
        (r"Cannot open C:/Users/someone/x.json", "Cannot open <path>"),
        (r"share \\server\share\f failed", "share <path> failed"),
        (r"verbatim \\?\D:\SugarSubstitute\releases\2", "verbatim <path>"),
        ("home /home/someone/.config/x", "home <path>"),
        (r"relative user\settings.json and releases/2.0.0/app.exe", "relative <path> and <path>"),
        (
            r"profile ~/x and %USERPROFILE%\Downloads and $HOME/x",
            "profile <path> and <path> and <path>",
        ),
        (r"dot .\user\x and ../x/y", "dot <path> and <path>"),
    ];
    for (text, expected) in cases {
        assert_eq!(redacted(text), expected, "{text}");
    }
}

#[test]
fn a_path_with_spaces_never_leaves_a_user_name_behind() {
    let quoted = redacted(r#"Cannot open "C:\Users\Jo Ann Smith\SugarSubstitute\x.json": denied"#);
    assert_eq!(quoted, "Cannot open <path> denied");
    let debug = redacted(r#"Cannot open "C:\\Users\\Jo Ann\\x.json" twice"#);
    assert_eq!(debug, "Cannot open <path> twice");
    let unquoted = redacted(r"Cannot open C:\Users\Jo Ann\SugarSubstitute\x.json now");
    assert_eq!(unquoted, "Cannot open <path> now");
    for text in [quoted, debug, unquoted] {
        assert!(!text.contains("Jo") && !text.contains("Ann") && !text.contains("Smith"));
    }
}

#[test]
fn an_unclosed_quoted_path_hides_the_rest_rather_than_leak_it() {
    assert_eq!(redacted(r#"open "C:\Users\Jo Ann Smith failed"#), "open <path>");
}

#[test]
fn network_addresses_become_one_placeholder() {
    let cases = [
        ("via https://updates.example.test/a?ticket=1 done", "via <address> done"),
        ("connect 127.0.0.1:8188 refused", "connect <address> refused"),
        ("connect 192.168.1.20 refused", "connect <address> refused"),
        ("connect [::1]:8188 and fe80::1 refused", "connect <address> and <address> refused"),
        ("proxy proxy.example.test:3128 refused", "proxy <address> refused"),
        ("host localhost:8188 down", "host <address> down"),
        ("mail someone@example.test bounced", "mail <address> bounced"),
    ];
    for (text, expected) in cases {
        assert_eq!(redacted(text), expected, "{text}");
    }
}

#[test]
fn credentials_and_their_values_are_redacted_wherever_the_value_sits() {
    let cases = [
        ("query token=abc password=hunter2", "query <redacted>"),
        ("header Authorization: Bearer abc.def.ghi rejected", "header <redacted> rejected"),
        ("auth Bearer eyJhbGciOi rejected", "auth Bearer <redacted> rejected"),
        ("login password: hunter2 refused", "login <redacted> refused"),
        ("cookie: sid=1 set", "<redacted> set"),
        ("api key=ABC refused", "api <redacted> refused"),
        ("token ghp_abcdef0123 refused", "token <redacted> refused"),
        ("key sk2ABCdefGHIjklMNOpqrSTUvwxYZ0123456 refused", "key <redacted> refused"),
        (
            "digest 9b4d83f76b899d8b722f1e4d9ac52ab61835e53f7b6f5602b0e5339ba91d9b59 differs",
            "digest <redacted> differs",
        ),
        ("jwt eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.abcdefghij seen", "jwt <redacted> seen"),
    ];
    for (text, expected) in cases {
        assert_eq!(redacted(text), expected, "{text}");
    }
}

#[test]
fn ordinary_diagnostic_words_stay_readable() {
    let cases = [
        "Update repository download failed",
        "Digest mismatch for SugarSubstitute-0.5.0-x86_64-pc-windows-msvc.zip",
        "protocol native-update-preparation-invocation-v2 refused",
        "basic checks and/or retries failed at 50% after 2.0.0",
        "Step: Downloading the update",
        "下载已停止，连接被重置",
    ];
    for text in cases {
        assert_eq!(redacted(text), text);
    }
}

#[test]
fn control_and_direction_characters_never_reach_a_report() {
    let text = redacted("line\none\ttab\u{1b}[31m red \u{202e}gnp.exe\u{0}");
    assert_eq!(text, "line one tab[31m red gnp.exe");
}

#[test]
fn placeholders_survive_unchanged_and_repeats_collapse() {
    assert_eq!(redacted("<path> <path> <address> <redacted>"), "<path> <address> <redacted>");
    assert_eq!(redacted(r"C:\a D:\b E:\c"), "<path>");
}

#[test]
fn results_are_bounded_at_a_character_boundary() {
    let long = "é".repeat(super::super::MAX_CAUSE_BYTES);
    let text = redacted(&long);
    assert!(text.len() <= super::super::MAX_CAUSE_BYTES);
    assert!(text.chars().all(|c| c == 'é'));
}
