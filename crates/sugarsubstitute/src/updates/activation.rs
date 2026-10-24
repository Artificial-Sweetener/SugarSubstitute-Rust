// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enter authenticated cached preparation with the once-captured native creator channel.
use std::{error::Error, sync::Arc};
use sugarsubstitute_supervision::application_launch::broker::ActivationSender;

/// Compiled distribution configuration supplies repository authority; entry supplies local custody.
pub(super) fn run(source: Arc<ActivationSender>) -> Result<(), Box<dyn Error>> {
    let root = source.installation().to_owned();
    let configuration = super::configuration(
        super::EMBEDDED,
        root.clone(),
        root.join(".update-workspace"),
        std::time::Duration::from_secs(30),
    )?
    .ok_or("Startup activation requires a configured update engine")?;
    crate::stable_launcher::activate(source, std::env::current_exe()?, configuration.repository)
}
