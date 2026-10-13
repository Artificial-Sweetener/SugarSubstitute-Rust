// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare bounded configuration and actual environment under one original owned job.

use super::options::ProductionEntryOptions;
use crate::job_sequence::JobSequence;
use std::{error::Error, fmt, fs::File};
use sugarsubstitute_comfy::configuration::{
    StartupConfigurationError, parse_startup_configuration,
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

#[path = "configuration/steps.rs"]
mod steps;
use steps::{ReadFailure, StepFailure, completed_step, read_bounded};

/// Original known retained storage; the operating-system block extent is unmeasured.
pub(crate) const CONFIGURATION_RESERVATION_BYTES: u64 = 4 * 1024 * 1024;

/// Private intermediate never escapes independently of the actual preparation owner.
struct LoadedConfiguration {
    options: ProductionEntryOptions,
    configuration: StartupConfiguration,
}

/// Preserve returned primary failures and mandatory genuine post-control causes.
pub(crate) enum ConfigurationCause {
    Control(EntryPreparationError),
    Read { source: std::io::Error, after: Option<EntryPreparationError> },
    Schema { source: Box<StartupConfigurationError>, after: Option<EntryPreparationError> },
    Capacity(std::collections::TryReserveError),
    TooLarge,
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
        }
    }
}
impl ConfigurationCause {
    fn read(outcome: ReadFailure<EntryPreparationError>) -> Self {
        match outcome {
            ReadFailure::Control(cause) => Self::Control(cause),
            ReadFailure::Read { source, after } => Self::Read { source, after },
            ReadFailure::Capacity(cause) => Self::Capacity(cause),
            ReadFailure::TooLarge => Self::TooLarge,
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
    options: ProductionEntryOptions,
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
            let LoadedConfiguration { options, configuration } =
                load_configuration(&context, options)?;
            let captured =
                context.capture_environment().map_err(ConfigurationCause::Environment)?;
            captured
                .into_request(
                    configuration,
                    options.package,
                    options.metadata,
                    options.cubes,
                    options.script,
                    options.profile,
                )
                .map_err(ConfigurationCause::Environment)
        })
        .map_err(|rejected| Box::new(rejected.error) as Box<dyn Error>)
}

fn load_configuration(
    context: &EntryPreparationContext,
    options: ProductionEntryOptions,
) -> Result<LoadedConfiguration, ConfigurationCause> {
    context.check().map_err(ConfigurationCause::Control)?;
    let opened = File::open(&options.configuration);
    let source =
        completed_step(opened, || context.observe_control().map(|_| ())).map_err(|failure| {
            match failure {
                StepFailure::Primary { source, after } => {
                    ConfigurationCause::Read { source, after }
                }
                StepFailure::Control(cause) => ConfigurationCause::Control(cause),
            }
        })?;
    let bytes = read_bounded(
        source,
        || context.check().map(|_| ()),
        || context.observe_control().map(|_| ()),
    )
    .map_err(ConfigurationCause::read)?;
    context.check().map_err(ConfigurationCause::Control)?;
    let parsed = parse_startup_configuration(&bytes);
    let configuration =
        completed_step(parsed, || context.observe_control().map(|_| ())).map_err(|failure| {
            match failure {
                StepFailure::Primary { source, after } => {
                    ConfigurationCause::Schema { source: Box::new(source), after }
                }
                StepFailure::Control(cause) => ConfigurationCause::Control(cause),
            }
        })?;
    Ok(LoadedConfiguration { options, configuration })
}

#[cfg(test)]
#[path = "configuration/tests.rs"]
mod tests;
