// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose the bounded preparation role from verified installed bytes without application custody.
mod provisioning;
use super::*;
use crate::setup_storage::{PreparationImage, TrustFailure, UpdaterProbeTransfer};
pub use provisioning::ProvisioningEndpoint;
use provisioning::ProvisioningLaunch;
use std::{ffi::OsString, io, os::windows::io::AsRawHandle, path::Path};
use sugarsubstitute_contracts::{configuration::LaunchConfiguration, lifecycle::CoordinatorKey};
use sugarsubstitute_execution::windows_purpose_pipe::{PurposeEnrollment, PurposePipeBootstrap};

/// The update-attempt channel a preparation helper inherits: the read-only bootstrap section
/// and the enrollment of the suspended helper. Both come from the launching parent's channel.
pub struct UpdateAttemptPins {
    /// Bootstrap of the parent's update-attempt channel.
    pub bootstrap: PurposePipeBootstrap,
    /// Enrollment that binds the suspended helper to that channel before it resumes.
    pub enrollment: PurposeEnrollment,
}

/// Move file custody into the native family before creation and retain it through settlement.
pub(crate) struct PreparationLaunch {
    pub(crate) owner: CoordinatorKey,
    pub(crate) payload: PreparationPayload,
}

/// Each helper retains its own authenticated bytes; probes additionally retain trial exclusion.
pub(crate) enum PreparationPayload {
    Image { _image: Box<PreparationImage> },
    Probe(Box<UpdaterProbeTransfer>),
    Provisioning(Box<ProvisioningLaunch>),
}
impl PreparationLaunch {
    /// Peak boxed role metadata charged to the same original native family allocation.
    pub(crate) const fn storage_bytes() -> usize {
        let existing = Self::image_storage_bytes();
        if size_of::<Self>() + size_of::<ProvisioningLaunch>() > existing {
            size_of::<Self>() + size_of::<ProvisioningLaunch>()
        } else {
            existing
        }
    }
    /// Compare existing source roles before including provisioning resources.
    const fn image_storage_bytes() -> usize {
        size_of::<Self>()
            + if size_of::<PreparationImage>() > size_of::<UpdaterProbeTransfer>() {
                size_of::<PreparationImage>()
            } else {
                size_of::<UpdaterProbeTransfer>()
            }
    }
    /// Transfer provisional command funding to the accepted native creation allocation.
    pub(crate) fn transfer_input_memory(&mut self) {
        if let PreparationPayload::Provisioning(value) = &mut self.payload {
            value.transfer_input_memory();
        }
    }
    /// Retain results whose owners must perform storage finalization after native settlement.
    pub(crate) fn retains_settled_payload(&self) -> bool {
        matches!(self.payload, PreparationPayload::Probe(_) | PreparationPayload::Provisioning(_))
    }
    /// Borrow the actual provisioning role without granting it to unrelated process launches.
    pub(crate) fn provisioning(&self) -> Option<&ProvisioningLaunch> {
        match &self.payload {
            PreparationPayload::Provisioning(value) => Some(value),
            _ => None,
        }
    }
    /// Borrow probe authority without granting it to ordinary preparation helpers.
    pub(crate) fn probe(&self) -> Option<&UpdaterProbeTransfer> {
        match &self.payload {
            PreparationPayload::Probe(probe) => Some(probe),
            PreparationPayload::Image { .. } | PreparationPayload::Provisioning(_) => None,
        }
    }
}

impl WindowsLaunchInput {
    /// Whether this input retains an updater trial that requires explicit withdrawal if unused.
    /// This describes owned custody; it grants no launch or process-health authority.
    pub fn is_updater_probe(&self) -> bool {
        self.preparation.as_deref().and_then(PreparationLaunch::probe).is_some()
    }

    /// Compose one preparation helper from this coordinator's verified running generation.
    /// No run locks, application leases or other transport authority are inherited; the only
    /// inherited handle is the optional update-attempt channel's read-only bootstrap section,
    /// whose suspended helper is enrolled before it resumes.
    /// The caller supplies the environment and locally selected workspace.
    /// Call on an admitted worker; the image keeps its original admitted memory charge.
    /// # Errors
    /// Rejects invalid paths, command capacity, environment storage and attempt pins that belong
    /// to another coordinator, purpose or session. Failure disposes the supplied image and pins;
    /// success transfers them into the registry's eventual native family.
    pub fn preparation(
        owner: CoordinatorKey,
        image: PreparationImage,
        workspace: &Path,
        environment: LaunchEnvironment,
        attempt: Option<UpdateAttemptPins>,
    ) -> io::Result<Self> {
        use sugarsubstitute_contracts::application_launch::updater::{
            PREPARATION_ARGUMENT, preparation::ATTEMPT_ARGUMENT,
        };
        LaunchConfiguration::validate_path(workspace).map_err(io::Error::other)?;
        let mut arguments = vec![
            OsString::from(PREPARATION_ARGUMENT),
            image.installation().as_os_str().to_owned(),
            workspace.as_os_str().to_owned(),
        ];
        if let Some(pins) = &attempt {
            let bootstrap = &pins.bootstrap;
            if bootstrap.key().update_attempt().is_none_or(|channel| channel.owner() != owner)
                || !bootstrap.matches_enrollment(&pins.enrollment)
            {
                return Err(io::Error::other("Update attempt channel belongs to another launch"));
            }
            let deadline = bootstrap
                .deadline()
                .ok_or_else(|| io::Error::other("Update attempt handshake is unavailable"))?;
            let token = bootstrap.borrow_inheritance().section.as_raw_handle() as usize;
            arguments.extend([
                OsString::from(ATTEMPT_ARGUMENT),
                OsString::from(token.to_string()),
                OsString::from(deadline.ticks().to_string()),
                OsString::from(deadline.frequency().to_string()),
            ]);
        }
        let command =
            WindowsLaunchCommand::from_os(&image.executable(), image.directory(), &arguments)
                .map_err(io::Error::other)?;
        let mut input = Self::new(command, environment).map_err(io::Error::other)?;
        if let Some(UpdateAttemptPins { bootstrap, enrollment }) = attempt {
            input
                .with_purpose_pipe(&mut Some(bootstrap), &mut Some(enrollment))
                .map_err(io::Error::other)?;
        }
        input.preparation = Some(Box::new(PreparationLaunch {
            owner,
            payload: PreparationPayload::Image { _image: Box::new(image) },
        }));
        Ok(input)
    }
}

impl WindowsLaunchInput {
    /// Compose a contained updater probe with only its retained exclusion handle inherited.
    /// The image, trial and original memory leases remain owned through process settlement.
    /// Execute on an admitted worker. Failure leaves `probe` populated for retry or withdrawal;
    /// success transfers it into the input and clears the caller's option.
    /// # Errors
    /// Rejects invalid command or environment capacity before any native process effect.
    pub fn updater_probe(
        owner: CoordinatorKey,
        probe: &mut Option<UpdaterProbeTransfer>,
        environment: LaunchEnvironment,
    ) -> io::Result<Self> {
        let original = probe.as_ref().ok_or_else(|| io::Error::other("Updater probe absent"))?;
        let command = WindowsLaunchCommand::from_os(
            &original.executable(),
            &original.directory(),
            &original.arguments()?,
        )
        .map_err(io::Error::other)?;
        let mut input = Self::new(command, environment).map_err(io::Error::other)?;
        let probe = probe.take().ok_or_else(|| io::Error::other("Updater probe absent"))?;
        input.preparation = Some(Box::new(PreparationLaunch {
            owner,
            payload: PreparationPayload::Probe(Box::new(probe)),
        }));
        Ok(input)
    }

    /// Withdraw an updater probe whose input has not transferred into the registry.
    /// The input becomes permanently unlaunchable before any fallible journal operation.
    /// Call on an admitted storage worker; retain this input until success or retry.
    /// # Errors
    /// Rejects other launch roles and preserves storage/cancellation failures with original custody.
    pub fn withdraw_updater_probe(
        &mut self,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), TrustFailure> {
        let Some(launch) = &mut self.preparation else {
            return Err(io::Error::other("Updater probe absent").into());
        };
        let PreparationPayload::Probe(probe) = &mut launch.payload else {
            return Err(io::Error::other("Launch input is not an updater probe").into());
        };
        probe.withdraw_before_launch(context)
    }
}
