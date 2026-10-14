// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pump the authenticated independent custody helper without product windows.
#[path = "backend_custody/capture.rs"]
mod capture;
#[path = "backend_custody/cleanup.rs"]
mod cleanup;
#[path = "backend_custody/lifetime.rs"]
mod lifetime;
#[path = "backend_custody/role.rs"]
mod role;
#[path = "backend_custody/terminal.rs"]
mod terminal;
use std::{error::Error, ffi::OsString};
/// Parse and run only this dedicated headless role through the existing desktop pump.
pub(super) fn run(arguments: impl IntoIterator<Item = OsString>) -> Result<(), Box<dyn Error>> {
    let mut lifetime = lifetime::Lifetime::new(role::Role::parse(arguments)?);
    sugarsubstitute_desktop::run_headless(&mut lifetime)?;
    Ok(())
}
