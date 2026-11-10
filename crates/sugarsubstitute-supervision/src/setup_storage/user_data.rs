// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Settle verified installation defaults before the separate sealed environment capture.
//! Original launch bytes and native input pins remain owned until capture or original failure
//! retirement. External selections never acquire installation-owned storage from their spelling.
mod backend;
mod controls;
mod host;
mod installation;
mod invocation;
pub use invocation::SourceLaunchSettlement;
mod originals;
mod preserved_diagnostics;
pub use preserved_diagnostics::{
    PRESERVED_DIAGNOSTIC_SOURCE_BYTES, PreservedDiagnosticEntry, PreservedDiagnosticSource,
};
mod runtime;
#[cfg(test)]
mod tests;
use crate::durable_storage::check;
use controls::Controls;
use installation::InstallationData;
use std::{error::Error, fmt, io, path::Path};
use sugarsubstitute_configuration::paths::{ConfigurationSource, StartupPaths};
use sugarsubstitute_contracts::{
    configuration::{LaunchConfiguration, StartupConfiguration},
    execution::{JobContext, MemoryClaim},
};

/// One finite job precedes four-MiB capture, using the same original setup deadline.
pub const LAUNCH_DATA_PREPARATION_BYTES: u64 = 32 * 1024 * 1024;
const RETAINED_BYTES: u64 = 4 * 1024 * 1024;
/// Memory a coordinator keeps charged for its whole life after launch data settles. An execution
/// owner declares this beside its standing working capacity, so retained custody never takes
/// capacity from the readiness and lifecycle jobs that run beside it.
pub const LAUNCH_DATA_LIFETIME_BYTES: u64 = invocation::LIFETIME_BYTES;

/// The native archive owner classifies bytes without associating them with a workflow/session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoricalArchiveDisposition {
    /// The existing archive integrity owner accepted its complete encoding.
    Validated,
    /// Rejected or unknown bytes remain a preserved original and grant no document association.
    PreservedOriginal,
}

/// Explicit directory selections observed by the Comfy option owner, never ownership evidence.
#[derive(Default)]
pub struct ExplicitBackendDirectories {
    /// A selected base also preserves ComfyUI's derived defaults.
    pub base: bool,
    /// Preserve the selected user settings/workflow location.
    pub user: bool,
    /// Preserve selected temporary storage.
    pub temporary: bool,
    /// Preserve selected input storage.
    pub input: bool,
    /// Preserve selected output storage.
    pub output: bool,
    /// Preserve selected model storage.
    pub models: bool,
}

struct Custody {
    _memory: MemoryClaim,
    controls: Controls,
    installation: Option<InstallationData>,
    _legacy: Option<runtime::LegacySources>,
    invocation: Option<invocation::InvocationSnapshot>,
}

/// Settled inert startup intent plus private original-file/account custody.
pub struct PreparedStartup {
    paths: StartupPaths,
    configuration: StartupConfiguration,
    custody: Custody,
}
/// Original native input pins and admitted memory, retained by the composing startup lifetime.
/// This opaque owner exposes no file handle, path admission or process authority.
pub struct StartupDataCustody {
    originals: Option<Custody>,
    passive: Option<invocation::InvocationSnapshot>,
}
impl StartupDataCustody {
    /// Borrow only complete immutable matching data for the same sealed request.
    pub fn originals_reference(
        &self,
    ) -> Option<sugarsubstitute_contracts::coordinator_entry::EntryOriginalsReference> {
        self.passive.as_ref().and_then(invocation::InvocationSnapshot::reference)
    }
    /// Successful sealed adoption ends mutable-original/selection exclusion promptly.
    /// Passive copy custody remains with this owner through observed handoff/retirement.
    pub fn original_inputs_adopted(&mut self) {
        self.originals = None;
    }
}
impl PreparedStartup {
    /// Transfer inert capture input separately from the opaque original-custody owner.
    /// The composing lifetime must retain the latter through capture terminal observation,
    /// including admission rejection, cancellation and the original failure's retirement.
    pub fn into_capture_parts(self) -> (StartupPaths, StartupConfiguration, StartupDataCustody) {
        let Self { paths, configuration, mut custody } = self;
        let passive = custody.invocation.take();
        (paths, configuration, StartupDataCustody { originals: Some(custody), passive })
    }
}

/// Preserve the first native/schema cause and genuine later controls with original custody.
pub struct LaunchDataFailure {
    source: io::Error,
    after: Option<io::Error>,
    _custody: Option<Box<Custody>>,
}
impl LaunchDataFailure {
    /// Borrow actual post-operation cancellation/deadline failure without replacing the first cause.
    pub fn post_control(&self) -> Option<&io::Error> {
        self.after.as_ref()
    }
}
impl fmt::Debug for LaunchDataFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaunchDataFailure")
            .field("post_control", &self.after.is_some())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for LaunchDataFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Startup data preparation failed")
    }
}
impl Error for LaunchDataFailure {
    /// `io::Error::source` skips a wrapped custom error, so expose that wrapped cause itself:
    /// callers and diagnostics keep the typed schema/ownership failure, not only its own cause.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self.source.get_ref() {
            Some(wrapped) => Some(wrapped),
            None => Some(&self.source),
        }
    }
}

/// Read exact inputs and migrate only feature-verified product defaults on an admitted worker.
/// The option callback belongs to Comfy's existing grammar owner and returns inert selections.
/// No deadline is created/renewed here. Retain the result through sealed capture; original files,
/// interrupted stages and complete recovery copies are never cleaned up by this owner.
/// # Errors
/// Rejects uncertain installation/provisioning ownership, live old writers, links/reparses,
/// unsupported/corrupt controls, overload, changing source bytes and destination conflicts.
pub fn prepare_launch_data<E: Error + Send + Sync + 'static>(
    image_directory: &Path,
    source: ConfigurationSource,
    selected: impl FnOnce(&LaunchConfiguration) -> Result<ExplicitBackendDirectories, E>,
    mut archives: impl FnMut(&Path, &JobContext) -> io::Result<HistoricalArchiveDisposition>,
    context: &JobContext,
) -> Result<PreparedStartup, LaunchDataFailure> {
    let memory = context.memory().claim(RETAINED_BYTES).map_err(|cause| LaunchDataFailure {
        source: io::Error::other(cause),
        after: None,
        _custody: None,
    })?;
    let mut custody = Custody {
        _memory: memory,
        controls: Controls::default(),
        installation: None,
        _legacy: None,
        invocation: None,
    };
    let result = (|| {
        check(context)?;
        // Reading/parsing originals has no storage-creation effect and preserves its actual
        // schema/read failure before later native provenance checks. Every write follows capture.
        let (paths, configuration) = custody.controls.resolve(source, context)?;
        custody.invocation = Some(invocation::InvocationSnapshot::begin(image_directory, context)?);
        custody.installation = InstallationData::open(image_directory, context)?;
        let Some(installation) = &custody.installation else {
            custody
                .invocation
                .as_mut()
                .ok_or_else(|| crate::durable_storage::invalid("Actual invocation custody absent"))?
                .publish(&custody.controls, &paths, context)?;
            return Ok((paths, configuration));
        };
        // The protected selected generation now retains the same verified native ancestry.
        // Installed migration keeps its separate first-recovery snapshot, not a source handoff.
        custody.invocation = None;
        custody.controls.ready = Some(controls::ControlFile::read(
            &installation.generation.path().join("installation-ready.json"),
            1024 * 1024,
            context,
        )?);
        originals::preserve(installation, &custody.controls, context)?;
        custody._legacy = Some(runtime::settle(installation, context)?);
        let paths = host::settle(installation, paths, &mut archives, context)?;
        let legacy = custody
            ._legacy
            .as_mut()
            .ok_or_else(|| crate::durable_storage::invalid("Historical source custody absent"))?;
        let configuration =
            backend::settle(installation, configuration, selected, legacy, context)?;
        Ok((paths, configuration))
    })();
    let after = check(context).err();
    match (result, after) {
        (Ok((paths, configuration)), None) => Ok(PreparedStartup { paths, configuration, custody }),
        (Err(source), after) => {
            Err(LaunchDataFailure { source, after, _custody: Some(Box::new(custody)) })
        }
        (Ok(_), Some(source)) => {
            Err(LaunchDataFailure { source, after: None, _custody: Some(Box::new(custody)) })
        }
    }
}
