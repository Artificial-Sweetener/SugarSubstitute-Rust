// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read installed launch paths on the existing bounded preparation worker.
use super::*;
/// Resolve installed configuration after the Root splash has presented its first frame.
pub(super) fn resolve(
    context: &EntryPreparationContext,
    source: ConfigurationSource,
) -> Result<StartupPaths, ConfigurationCause> {
    let path = match source {
        ConfigurationSource::Explicit(paths) => return Ok(paths),
        ConfigurationSource::Installed(path) => path,
    };
    let bytes = read_preparation_file(context, &path, MAX_STARTUP_CONFIGURATION_BYTES)
        .map_err(ConfigurationCause::read)?;
    let paths = sugarsubstitute_configuration::descriptor::LaunchDescriptor::parse(&bytes)
        .map_err(|cause| ConfigurationCause::Installed(Box::new(cause)))?
        .into_startup_paths()
        .map_err(|cause| ConfigurationCause::Installed(Box::new(cause)))?;
    context.check().map_err(ConfigurationCause::Control)?;
    Ok(paths)
}
