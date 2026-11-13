// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project bounded inert fields through the sole Comfy command and environment owners.
use super::capacity;
use std::{
    error::Error,
    ffi::OsString,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::{
    configuration::{BackendDataDisposition, LaunchConfiguration, LocaleId, ProfileId},
    coordinator_entry::{EntryConnectionView, EntryLaunchRequestView, EntryWindowsPathView},
    lifecycle::BackendTarget,
};
use sugarsubstitute_supervision::{
    LaunchEnvironment, installed_frontend::FrontendInstallationInput, setup_storage::BackendStorage,
};
/// Inert owned projection with measured capacity; native admission stays with the composing run.
pub(super) struct Fields {
    pub(super) allocated_bytes: u64,
    pub(super) requested_profile: PathBuf,
    pub(super) profile_units: Vec<u16>,
    pub(super) profile: ProfileId,
    pub(super) locale: LocaleId,
    pub(super) target: BackendTarget,
    pub(super) frontend: FrontendInstallationInput,
    pub(super) backend_environment: Option<LaunchEnvironment>,
    pub(super) backend_data_disposition: BackendDataDisposition,
    pub(super) desktop_environment: LaunchEnvironment,
}

/// Convert one sealed intent after bounded preflight, using the native owner's storage selection.
/// The optional profile comes from completed source settlement and never rewrites the sealed body.
pub(super) fn project<'storage>(
    view: EntryLaunchRequestView<'_>,
    capture: &PathBuf,
    available: u64,
    settled_profile: Option<&Path>,
    backend_storage: impl FnOnce(
        &LaunchConfiguration,
    ) -> Result<BackendStorage<'storage>, Box<dyn Error + Send + Sync>>,
) -> Result<Fields, Box<dyn Error + Send + Sync>> {
    capacity::preflight(view, capture, available, settled_profile)?;
    let profile_units = match settled_profile {
        Some(profile) => {
            let mut units = Vec::new();
            units.try_reserve_exact(profile.as_os_str().encode_wide().count())?;
            units.extend(profile.as_os_str().encode_wide());
            units
        }
        None => collect_units(view.profile_directory())?,
    };
    let requested_profile = PathBuf::from(OsString::from_wide(&profile_units));
    LaunchConfiguration::validate_path(&requested_profile)?;
    let profile = ProfileId::new(copy_string(view.profile())?)?;
    let frontend = view.frontend();
    let frontend_paths = [
        path(frontend.package_directory())?,
        path(frontend.metadata_file())?,
        path(frontend.cube_directory())?,
        path(frontend.backend_script())?,
    ];
    let frontend_bytes = capacity::frontend(&frontend_paths)?;
    let [package, metadata, cube, script] = frontend_paths;
    let (target, backend_storage, backend_data_disposition) = match view.connection() {
        EntryConnectionView::Attached(origin) => {
            (BackendTarget::Attached(origin), None, BackendDataDisposition::External)
        }
        EntryConnectionView::Managed { root, interpreter, arguments, port } => {
            let mut owned = Vec::new();
            owned.try_reserve_exact(arguments.len())?;
            for argument in arguments.iter() {
                owned.push(copy_string(argument)?);
            }
            let requested =
                LaunchConfiguration::new(path(root)?, path(interpreter)?, owned, port.get())?;
            // Schema-one arguments are extras. Resolve identity from the sole
            // Comfy owner's complete command, while the original data claim is held.
            let storage = backend_storage(&requested)?;
            let disposition = match storage {
                BackendStorage::External => BackendDataDisposition::External,
                BackendStorage::InstallationOwned(_) => BackendDataDisposition::InstallationOwned,
            };
            (
                BackendTarget::Managed(sugarsubstitute_comfy::launch::effective_managed_launch(
                    &requested,
                    disposition,
                    &cube,
                )?),
                Some(storage),
                disposition,
            )
        }
    };
    let frontend = FrontendInstallationInput::new(package, metadata, cube, script)?;
    let mut environment = Vec::new();
    environment.try_reserve_exact(view.environment().unit_len())?;
    environment.extend(view.environment().units());
    let storage = requested_profile.parent().ok_or("Profile storage parent missing")?;
    let desktop_environment = LaunchEnvironment::for_local_storage(&environment, storage)?;
    let backend_environment = match &target {
        BackendTarget::Managed(configuration) => Some(LaunchEnvironment::for_managed_backend(
            &environment,
            configuration,
            backend_storage.ok_or("Managed backend storage selection missing")?,
        )?),
        BackendTarget::Attached(_) => None,
    };
    let mut fields = Fields {
        allocated_bytes: 0,
        requested_profile,
        profile_units,
        profile,
        locale: view.presentation().locale(),
        target,
        frontend,
        backend_environment,
        backend_data_disposition,
        desktop_environment,
    };
    fields.allocated_bytes = capacity::data(&fields, frontend_bytes)?;
    Ok(fields)
}
/// Reserve exact retained text without hidden growth or normalization.
fn copy_string(value: &str) -> Result<String, std::collections::TryReserveError> {
    let mut result = String::new();
    result.try_reserve_exact(value.len())?;
    result.push_str(value);
    Ok(result)
}
/// Preserve exact Windows units, including unpaired surrogates, in one reserved buffer.
fn collect_units(
    value: EntryWindowsPathView<'_>,
) -> Result<Vec<u16>, std::collections::TryReserveError> {
    let mut units = Vec::new();
    units.try_reserve_exact(value.unit_len())?;
    units.extend(value.units());
    Ok(units)
}
/// Reapply the existing lexical path owner after lossless borrowed-unit conversion.
pub(super) fn path(
    value: EntryWindowsPathView<'_>,
) -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    let units = collect_units(value)?;
    let result = PathBuf::from(OsString::from_wide(&units));
    LaunchConfiguration::validate_path(&result)?;
    Ok(result)
}
