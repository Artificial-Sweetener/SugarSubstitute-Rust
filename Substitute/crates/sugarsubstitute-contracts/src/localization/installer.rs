// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Complete typed installer copy serialized into the local presentation document.

use serde::Serialize;
mod text;
pub use text::InstallerText;
mod status;
pub use status::{FailureMessage, ProgressMessage};
mod en;
pub(super) use en::EN;
mod zh_hans;
pub(super) use zh_hans::ZH_HANS;
mod ja;
pub(super) use ja::JA;
mod ko;
pub(super) use ko::KO;
mod es;
pub(super) use es::ES;

/// Every field is required for every release-enabled locale.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct InstallerMessages {
    /// Accessible description of the immutable product wordmark artwork.
    pub wordmark_description: &'static str,
    /// Actionable feedback for a relative installation folder.
    pub choose_absolute_folder: &'static str,
    /// Actionable feedback for invalid folder or runtime choices.
    pub review_choices: &'static str,
    /// Installer title copy.
    pub title: &'static str,
    /// Installer close copy.
    pub close: &'static str,
    /// Installer language title copy.
    pub language_title: &'static str,
    /// Installer language description copy.
    pub language_description: &'static str,
    /// Installer location title copy.
    pub location_title: &'static str,
    /// Installer location description copy.
    pub location_description: &'static str,
    /// Installer install folder copy.
    pub install_folder: &'static str,
    /// Installer browse copy.
    pub browse: &'static str,
    /// Installer runtime title copy.
    pub runtime_title: &'static str,
    /// Installer managed title copy.
    pub managed_title: &'static str,
    /// Installer managed description copy.
    pub managed_description: &'static str,
    /// Installer attached title copy.
    pub attached_title: &'static str,
    /// Installer attached description copy.
    pub attached_description: &'static str,
    /// Installer channel copy.
    pub channel: &'static str,
    /// Installer version copy.
    pub version: &'static str,
    /// Installer hardware copy.
    pub hardware: &'static str,
    /// Installer mirrors copy.
    pub mirrors: &'static str,
    /// Installer comfy folder copy.
    pub comfy_folder: &'static str,
    /// Installer python copy.
    pub python: &'static str,
    /// Installer python help copy.
    pub python_help: &'static str,
    /// Installer folders title copy.
    pub folders_title: &'static str,
    /// Installer models copy.
    pub models: &'static str,
    /// Installer outputs copy.
    pub outputs: &'static str,
    /// Installer preservation copy.
    pub preservation: &'static str,
    /// Installer progress title copy.
    pub progress_title: &'static str,
    /// Installer long wait copy.
    pub long_wait: &'static str,
    /// Installer details copy.
    pub details: &'static str,
    /// Installer complete title copy.
    pub complete_title: &'static str,
    /// Installer back copy.
    pub back: &'static str,
    /// Installer continue copy.
    #[serde(rename = "continue")]
    pub continue_action: &'static str,
    /// Installer install copy.
    pub install: &'static str,
    /// Installer open copy.
    pub open: &'static str,
    /// Installer step copy.
    pub step: &'static str,
    /// Installer no environment copy.
    pub no_environment: &'static str,
    /// Installer retry copy.
    pub retry: &'static str,
    /// Installer cancelled copy.
    pub cancelled: &'static str,
    /// Installer cancelling copy.
    pub cancelling: &'static str,
    /// Installer worker stopped copy.
    pub worker_stopped: &'static str,
    /// Installer checking sugar cubes copy.
    pub checking_sugar_cubes: &'static str,
    /// Installer installing sugar cubes copy.
    pub installing_sugar_cubes: &'static str,
    /// Installer sugar cubes ready copy.
    pub sugar_cubes_ready: &'static str,
    /// Installer setup failed copy.
    pub setup_failed: &'static str,
    /// Installer configuration title managed copy.
    pub configuration_title_managed: &'static str,
    /// Installer configuration description managed copy.
    pub configuration_description_managed: &'static str,
    /// Installer configuration title attached copy.
    pub configuration_title_attached: &'static str,
    /// Installer configuration description attached copy.
    pub configuration_description_attached: &'static str,
    /// Installer select copy.
    pub select: &'static str,
    /// Installer selected copy.
    pub selected: &'static str,
    /// Installer progress description copy.
    pub progress_description: &'static str,
    /// Installer step language copy.
    pub step_language: &'static str,
    /// Installer step location copy.
    pub step_location: &'static str,
    /// Installer step runtime copy.
    pub step_runtime: &'static str,
    /// Installer step configuration copy.
    pub step_configuration: &'static str,
    /// Installer step folders copy.
    pub step_folders: &'static str,
    /// Installer step progress copy.
    pub step_progress: &'static str,
    /// Installer step complete copy.
    pub step_complete: &'static str,
    /// Installer hide details copy.
    pub hide_details: &'static str,
    /// Installer preparing installation copy.
    pub preparing_installation: &'static str,
    /// Installer step maintenance copy.
    pub step_maintenance: &'static str,
    /// Installer maintenance title copy.
    pub maintenance_title: &'static str,
    /// Installer maintenance description copy.
    pub maintenance_description: &'static str,
    /// Installer maintenance missing copy.
    pub maintenance_missing: &'static str,
    /// Installer maintain copy.
    pub maintain: &'static str,
    /// Installer maintenance launch missing copy.
    pub maintenance_launch_missing: &'static str,
    /// Installer recovery conflict copy.
    pub recovery_conflict: &'static str,
    /// Installer recovery incomplete copy.
    pub recovery_incomplete: &'static str,
    /// Installer recovery inventory bound copy.
    pub recovery_inventory_bound: &'static str,
    /// Installer cache full copy.
    pub cache_full: &'static str,
    /// Installer checking python copy.
    pub checking_python: &'static str,
    /// Installer select python copy.
    pub select_python: &'static str,
}

/// Resolve through the sole locale registry with canonical English fallback.
pub fn messages(locale: &str) -> &'static InstallerMessages {
    super::messages(locale).installer
}

impl InstallerMessages {
    /// Iterate all text for completeness validation without maintaining another key list.
    pub fn labels(&self) -> impl Iterator<Item = &'static str> {
        [
            self.wordmark_description,
            self.choose_absolute_folder,
            self.review_choices,
            self.title,
            self.close,
            self.language_title,
            self.language_description,
            self.location_title,
            self.location_description,
            self.install_folder,
            self.browse,
            self.runtime_title,
            self.managed_title,
            self.managed_description,
            self.attached_title,
            self.attached_description,
            self.channel,
            self.version,
            self.hardware,
            self.mirrors,
            self.comfy_folder,
            self.python,
            self.python_help,
            self.folders_title,
            self.models,
            self.outputs,
            self.preservation,
            self.progress_title,
            self.long_wait,
            self.details,
            self.complete_title,
            self.back,
            self.continue_action,
            self.install,
            self.open,
            self.step,
            self.no_environment,
            self.retry,
            self.cancelled,
            self.cancelling,
            self.worker_stopped,
            self.checking_sugar_cubes,
            self.installing_sugar_cubes,
            self.sugar_cubes_ready,
            self.setup_failed,
            self.configuration_title_managed,
            self.configuration_description_managed,
            self.configuration_title_attached,
            self.configuration_description_attached,
            self.select,
            self.selected,
            self.progress_description,
            self.step_language,
            self.step_location,
            self.step_runtime,
            self.step_configuration,
            self.step_folders,
            self.step_progress,
            self.step_complete,
            self.hide_details,
            self.preparing_installation,
            self.step_maintenance,
            self.maintenance_title,
            self.maintenance_description,
            self.maintenance_missing,
            self.maintain,
            self.maintenance_launch_missing,
            self.recovery_conflict,
            self.recovery_incomplete,
            self.recovery_inventory_bound,
            self.cache_full,
            self.checking_python,
            self.select_python,
        ]
        .into_iter()
    }
}

#[cfg(test)]
mod tests;
