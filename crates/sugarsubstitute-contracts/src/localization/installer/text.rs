// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Typed installer label selection for native page composition.
use super::InstallerMessages;

/// One canonical installer label; selection grants no presentation or native authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallerText {
    /// Select the wordmark description label.
    WordmarkDescription,
    /// Select the choose absolute folder label.
    ChooseAbsoluteFolder,
    /// Select the review choices label.
    ReviewChoices,
    /// Select the title label.
    Title,
    /// Select the close label.
    Close,
    /// Select the language title label.
    LanguageTitle,
    /// Select the language description label.
    LanguageDescription,
    /// Select the location title label.
    LocationTitle,
    /// Select the location description label.
    LocationDescription,
    /// Select the install folder label.
    InstallFolder,
    /// Select the browse label.
    Browse,
    /// Select the runtime title label.
    RuntimeTitle,
    /// Select the managed title label.
    ManagedTitle,
    /// Select the managed description label.
    ManagedDescription,
    /// Select the attached title label.
    AttachedTitle,
    /// Select the attached description label.
    AttachedDescription,
    /// Select the channel label.
    Channel,
    /// Select the version label.
    Version,
    /// Select the hardware label.
    Hardware,
    /// Select the mirrors label.
    Mirrors,
    /// Select the comfy folder label.
    ComfyFolder,
    /// Select the python label.
    Python,
    /// Select the python help label.
    PythonHelp,
    /// Select the folders title label.
    FoldersTitle,
    /// Select the models label.
    Models,
    /// Select the outputs label.
    Outputs,
    /// Select the preservation label.
    Preservation,
    /// Select the progress title label.
    ProgressTitle,
    /// Select the long wait label.
    LongWait,
    /// Select the details label.
    Details,
    /// Select the complete title label.
    CompleteTitle,
    /// Select the back label.
    Back,
    /// Select the install label.
    Install,
    /// Select the open label.
    Open,
    /// Select the step label.
    Step,
    /// Select the no environment label.
    NoEnvironment,
    /// Select the retry label.
    Retry,
    /// Select the cancelled label.
    Cancelled,
    /// Select the cancelling label.
    Cancelling,
    /// Select the worker stopped label.
    WorkerStopped,
    /// Select the checking sugar cubes label.
    CheckingSugarCubes,
    /// Select the installing sugar cubes label.
    InstallingSugarCubes,
    /// Select the sugar cubes ready label.
    SugarCubesReady,
    /// Select the setup failed label.
    SetupFailed,
    /// Select the configuration title managed label.
    ConfigurationTitleManaged,
    /// Select the configuration description managed label.
    ConfigurationDescriptionManaged,
    /// Select the configuration title attached label.
    ConfigurationTitleAttached,
    /// Select the configuration description attached label.
    ConfigurationDescriptionAttached,
    /// Select the select label.
    Select,
    /// Select the selected label.
    Selected,
    /// Select the progress description label.
    ProgressDescription,
    /// Select the step language label.
    StepLanguage,
    /// Select the step location label.
    StepLocation,
    /// Select the step runtime label.
    StepRuntime,
    /// Select the step configuration label.
    StepConfiguration,
    /// Select the step folders label.
    StepFolders,
    /// Select the step progress label.
    StepProgress,
    /// Select the step complete label.
    StepComplete,
    /// Select the hide details label.
    HideDetails,
    /// Select the preparing installation label.
    PreparingInstallation,
    /// Select the step maintenance label.
    StepMaintenance,
    /// Select the maintenance title label.
    MaintenanceTitle,
    /// Select the maintenance description label.
    MaintenanceDescription,
    /// Select the maintenance missing label.
    MaintenanceMissing,
    /// Select the maintain label.
    Maintain,
    /// Select the maintenance launch missing label.
    MaintenanceLaunchMissing,
    /// Select the recovery conflict label.
    RecoveryConflict,
    /// Select the recovery incomplete label.
    RecoveryIncomplete,
    /// Select the recovery inventory bound label.
    RecoveryInventoryBound,
    /// Select the cache full label.
    CacheFull,
    /// Select the checking python label.
    CheckingPython,
    /// Select the select python label.
    SelectPython,
    /// Select the continue action label.
    ContinueAction,
}
impl InstallerText {
    /// Borrow the selected label from the complete locale owned by the caller.
    pub fn text(self, copy: &InstallerMessages) -> &'static str {
        match self {
            Self::WordmarkDescription => copy.wordmark_description,
            Self::ChooseAbsoluteFolder => copy.choose_absolute_folder,
            Self::ReviewChoices => copy.review_choices,
            Self::Title => copy.title,
            Self::Close => copy.close,
            Self::LanguageTitle => copy.language_title,
            Self::LanguageDescription => copy.language_description,
            Self::LocationTitle => copy.location_title,
            Self::LocationDescription => copy.location_description,
            Self::InstallFolder => copy.install_folder,
            Self::Browse => copy.browse,
            Self::RuntimeTitle => copy.runtime_title,
            Self::ManagedTitle => copy.managed_title,
            Self::ManagedDescription => copy.managed_description,
            Self::AttachedTitle => copy.attached_title,
            Self::AttachedDescription => copy.attached_description,
            Self::Channel => copy.channel,
            Self::Version => copy.version,
            Self::Hardware => copy.hardware,
            Self::Mirrors => copy.mirrors,
            Self::ComfyFolder => copy.comfy_folder,
            Self::Python => copy.python,
            Self::PythonHelp => copy.python_help,
            Self::FoldersTitle => copy.folders_title,
            Self::Models => copy.models,
            Self::Outputs => copy.outputs,
            Self::Preservation => copy.preservation,
            Self::ProgressTitle => copy.progress_title,
            Self::LongWait => copy.long_wait,
            Self::Details => copy.details,
            Self::CompleteTitle => copy.complete_title,
            Self::Back => copy.back,
            Self::Install => copy.install,
            Self::Open => copy.open,
            Self::Step => copy.step,
            Self::NoEnvironment => copy.no_environment,
            Self::Retry => copy.retry,
            Self::Cancelled => copy.cancelled,
            Self::Cancelling => copy.cancelling,
            Self::WorkerStopped => copy.worker_stopped,
            Self::CheckingSugarCubes => copy.checking_sugar_cubes,
            Self::InstallingSugarCubes => copy.installing_sugar_cubes,
            Self::SugarCubesReady => copy.sugar_cubes_ready,
            Self::SetupFailed => copy.setup_failed,
            Self::ConfigurationTitleManaged => copy.configuration_title_managed,
            Self::ConfigurationDescriptionManaged => copy.configuration_description_managed,
            Self::ConfigurationTitleAttached => copy.configuration_title_attached,
            Self::ConfigurationDescriptionAttached => copy.configuration_description_attached,
            Self::Select => copy.select,
            Self::Selected => copy.selected,
            Self::ProgressDescription => copy.progress_description,
            Self::StepLanguage => copy.step_language,
            Self::StepLocation => copy.step_location,
            Self::StepRuntime => copy.step_runtime,
            Self::StepConfiguration => copy.step_configuration,
            Self::StepFolders => copy.step_folders,
            Self::StepProgress => copy.step_progress,
            Self::StepComplete => copy.step_complete,
            Self::HideDetails => copy.hide_details,
            Self::PreparingInstallation => copy.preparing_installation,
            Self::StepMaintenance => copy.step_maintenance,
            Self::MaintenanceTitle => copy.maintenance_title,
            Self::MaintenanceDescription => copy.maintenance_description,
            Self::MaintenanceMissing => copy.maintenance_missing,
            Self::Maintain => copy.maintain,
            Self::MaintenanceLaunchMissing => copy.maintenance_launch_missing,
            Self::RecoveryConflict => copy.recovery_conflict,
            Self::RecoveryIncomplete => copy.recovery_incomplete,
            Self::RecoveryInventoryBound => copy.recovery_inventory_bound,
            Self::CacheFull => copy.cache_full,
            Self::CheckingPython => copy.checking_python,
            Self::SelectPython => copy.select_python,
            Self::ContinueAction => copy.continue_action,
        }
    }
}
