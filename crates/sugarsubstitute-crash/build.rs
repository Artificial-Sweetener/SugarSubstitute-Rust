// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Link the same committed-dump parser archive built by the pinned native target.
fn main() {
    println!("cargo:rerun-if-env-changed=SUGAR_COMMITTED_PARSER_LIB");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && let Some(directory) = std::env::var_os("SUGAR_COMMITTED_PARSER_LIB")
    {
        println!("cargo:rustc-link-search=native={}", std::path::Path::new(&directory).display());
    }
}
