// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare bounded configuration and actual environment under one original owned job.

use super::options::ConfigurationSource;
use sugarsubstitute_configuration::paths::StartupPaths;
#[path = "configuration/installed.rs"]
mod installed;
use crate::job_sequence::JobSequence;
use std::{error::Error, fmt};
use sugarsubstitute_configuration::startup::{
    MAX_STARTUP_CONFIGURATION_BYTES, StartupConfigurationError, parse_startup_configuration,
};
use sugarsubstitute_contracts::{
    configuration::StartupConfiguration,
    coordinator_entry::EntrySetupBudget,
    execution::{JobClass, JobSpec, Operation, OwnedJob, Scope},
};
use sugarsubstitute_execution::{
    EntryEnvironmentFailure, EntryPreparationContext, EntryPreparationError,
    EntryPreparationFailure, EntryPreparationSuccess, EntryRequestInput, ExecutionClient,
};

use sugarsubstitute_supervision::{PreparationFileError, read_preparation_file};

/// Preserve a parser result while independently observing its original preparation controls.
enum ParseFailure<E, C> {
    Primary { source: E, after: Option<C> },
    Control(C),
}
/// The returned parser value/cause remains owned until the lazy observer finishes.
fn observe_parse<T, E, C>(
    outcome: Result<T, E>,
    after: impl FnOnce() -> Result<(), C>,
) -> Result<T, ParseFailure<E, C>> {
    let post = after();
    match (outcome, post) {
        (Err(source), after) => Err(ParseFailure::Primary { source, after: after.err() }),
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(cause)) => Err(ParseFailure::Control(cause)),
    }
}

/// Original known retained storage; the operating-system block extent is unmeasured.
pub(crate) const CONFIGURATION_RESERVATION_BYTES: u64 = 4 * 1024 * 1024;

/// Private intermediate never escapes independently of the actual preparation owner.
struct LoadedConfiguration {
    options: StartupPaths,
    configuration: StartupConfiguration,
}

/// Preserve returned primary failures and mandatory genuine post-control causes.
pub(crate) enum ConfigurationCause {
    Control(EntryPreparationError),
    Read { source: std::io::Error, after: Option<EntryPreparationError> },
    Schema { source: Box<StartupConfigurationError>, after: Option<EntryPreparationError> },
    Capacity(std::collections::TryReserveError),
    TooLarge,
    Installed(Box<dyn Error + Send + Sync>),
    Environment(EntryEnvironmentFailure),
}
impl fmt::Debug for ConfigurationCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigurationCause")
            .field("post_control", &self.post_control())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for ConfigurationCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Startup configuration preparation failed")
    }
}
impl Error for ConfigurationCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Control(cause) => Some(cause),
            Self::Read { source, .. } => Some(source),
            Self::Schema { source, .. } => Some(source.as_ref()),
            Self::Capacity(cause) => Some(cause),
            Self::Environment(cause) => Some(cause),
            Self::TooLarge => None,
            Self::Installed(cause) => Some(cause.as_ref()),
        }
    }
}
impl ConfigurationCause {
    fn read(outcome: PreparationFileError) -> Self {
        match outcome {
            PreparationFileError::Control(cause) => Self::Control(cause),
            PreparationFileError::Read { source, after } => Self::Read { source, after },
            PreparationFileError::Capacity(cause) => Self::Capacity(cause),
            PreparationFileError::TooLarge => Self::TooLarge,
        }
    }
    /// Borrow genuine post-control while leaving the primary source unchanged.
    pub(crate) fn post_control(&self) -> Option<&EntryPreparationError> {
        match self {
            Self::Read { after, .. } | Self::Schema { after, .. } => after.as_ref(),
            _ => None,
        }
    }
}
type PreparedRequestJob = OwnedJob<
    EntryPreparationSuccess<EntryRequestInput>,
    EntryPreparationFailure<ConfigurationCause>,
>;
/// Admit the concrete complete request factory on the process's existing execution owner.
/// # Errors
/// Rejects original admission before opening files or reading native environment.
pub(crate) fn submit(
    client: &ExecutionClient,
    scope: &Scope,
    jobs: &JobSequence,
    options: ConfigurationSource,
    original: EntrySetupBudget,
) -> Result<PreparedRequestJob, Box<dyn Error>> {
    let spec = JobSpec {
        id: jobs.allocate()?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(original.until()),
        reservation_bytes: CONFIGURATION_RESERVATION_BYTES,
        replacement: None,
    };
    client
        .submit_entry_preparation(scope, spec, original, move |context| {
            let options = installed::resolve(&context, options)?;
            let LoadedConfiguration { options, configuration } =
                load_configuration(&context, options)?;
            let captured =
                context.capture_environment().map_err(ConfigurationCause::Environment)?;
            let [_, package, metadata, cubes, script, profile] = options.into_paths();
            captured
                .into_request(configuration, package, metadata, cubes, script, profile)
                .map_err(ConfigurationCause::Environment)
        })
        .map_err(|rejected| Box::new(rejected.error) as Box<dyn Error>)
}

fn load_configuration(
    context: &EntryPreparationContext,
    options: StartupPaths,
) -> Result<LoadedConfiguration, ConfigurationCause> {
    let bytes =
        read_preparation_file(context, options.configuration(), MAX_STARTUP_CONFIGURATION_BYTES)
            .map_err(ConfigurationCause::read)?;
    context.check().map_err(ConfigurationCause::Control)?;
    let parsed = parse_startup_configuration(&bytes);
    let configuration =
        observe_parse(parsed, || context.observe_control().map(|_| ())).map_err(|failure| {
            match failure {
                ParseFailure::Primary { source, after } => {
                    ConfigurationCause::Schema { source: Box::new(source), after }
                }
                ParseFailure::Control(cause) => ConfigurationCause::Control(cause),
            }
        })?;
    Ok(LoadedConfiguration { options, configuration })
}

#[cfg(test)]
#[path = "configuration/tests.rs"]
mod tests;
