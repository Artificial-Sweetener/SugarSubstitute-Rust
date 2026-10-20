// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read installed launch paths on the existing bounded preparation worker.
use super::*;
use serde::Deserialize;

/// Versioned local installation descriptor; paths retain the explicit launch validator.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Descriptor {
    schema_version: u32,
    arguments: Vec<String>,
}

/// Resolve installed configuration after the Root splash has presented its first frame.
pub(super) fn resolve(
    context: &EntryPreparationContext,
    source: ConfigurationSource,
) -> Result<ProductionEntryOptions, ConfigurationCause> {
    let path = match source {
        ConfigurationSource::Explicit(paths) => return Ok(paths),
        ConfigurationSource::Installed(path) => path,
    };
    context.check().map_err(ConfigurationCause::Control)?;
    let opened = completed_step(File::open(path), || context.observe_control().map(|_| ()))
        .map_err(|failure| match failure {
            StepFailure::Primary { source, after } => ConfigurationCause::Read { source, after },
            StepFailure::Control(cause) => ConfigurationCause::Control(cause),
        })?;
    let bytes = read_bounded(
        opened,
        || context.check().map(|_| ()),
        || context.observe_control().map(|_| ()),
    )
    .map_err(ConfigurationCause::read)?;
    let paths = parse(&bytes).map_err(ConfigurationCause::Installed)?;
    context.check().map_err(ConfigurationCause::Control)?;
    Ok(paths)
}

/// Reject unknown versions, fields and options using the same public path grammar.
fn parse(bytes: &[u8]) -> Result<ProductionEntryOptions, Box<dyn Error + Send + Sync>> {
    let descriptor: Descriptor = serde_json::from_slice(bytes)?;
    if descriptor.schema_version != 1 {
        return Err("Unsupported installed launch descriptor".into());
    }
    Ok(ProductionEntryOptions::parse(
        descriptor.arguments.into_iter().map(std::ffi::OsString::from),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installed_paths_use_the_explicit_contract_and_reject_unknown_schema()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        let arguments: Vec<_> = [
            "--startup-config",
            "--frontend-package",
            "--frontend-metadata",
            "--sugarcubes-root",
            "--backend-script",
            "--profile-directory",
        ]
        .into_iter()
        .flat_map(|flag| [flag, r"E:\installed\value"])
        .collect();
        let bytes =
            serde_json::to_vec(&serde_json::json!({"schema_version":1,"arguments":arguments}))?;
        assert_eq!(parse(&bytes)?.configuration, std::path::Path::new(r"E:\installed\value"));
        for value in [
            serde_json::json!({"schema_version":2,"arguments":arguments}),
            serde_json::json!({"schema_version":1,"arguments":arguments,"extra":true}),
            serde_json::json!({"schema_version":1,"arguments":["--startup-config","relative"]}),
        ] {
            assert!(parse(&serde_json::to_vec(&value)?).is_err());
        }
        Ok(())
    }
}
