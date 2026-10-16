// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run the fresh subordinate without creating a product window or coordinator authority.
#[path = "recovery_supervisor/capture.rs"]
mod capture;
#[path = "recovery_supervisor/channel.rs"]
mod channel;
#[path = "recovery_supervisor/cleanup.rs"]
mod cleanup;
#[path = "recovery_supervisor/command.rs"]
mod command;
#[path = "recovery_supervisor/lifetime.rs"]
mod lifetime;
#[path = "recovery_supervisor/role.rs"]
pub(in crate::production) mod role;
use std::{error::Error, ffi::OsString};
/// Accept only the dedicated inherited role and retain its actual execution through settlement.
/// Root authority, profile/backend leases and direct UI protocol remain with the surviving Root.
pub(super) fn run(arguments: impl IntoIterator<Item = OsString>) -> Result<(), Box<dyn Error>> {
    let mut lifetime = lifetime::Lifetime::new(role::Role::parse(arguments)?);
    sugarsubstitute_desktop::run_headless(&mut lifetime)?;
    Ok(())
}
