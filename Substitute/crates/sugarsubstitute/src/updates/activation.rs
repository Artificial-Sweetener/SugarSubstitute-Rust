// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run this updater as the launch engine when the selected release needs a newer updater.
use std::error::Error;
use sugarsubstitute_supervision::application_launch::EngineLaunch;

/// Compiled distribution configuration supplies repository authority; entry supplies the
/// installation its creator named.
pub(super) fn run(launch: &EngineLaunch) -> Result<(), Box<dyn Error>> {
    let root = launch.installation().to_owned();
    let configuration = super::configuration(
        super::EMBEDDED,
        root.clone(),
        root.join(".update-workspace"),
        std::time::Duration::from_secs(30),
    )?
    .ok_or("The launch engine role requires a configured update engine")?;
    crate::stable_launcher::run_update_engine(
        root,
        std::env::current_exe()?,
        configuration.repository,
    )
}
