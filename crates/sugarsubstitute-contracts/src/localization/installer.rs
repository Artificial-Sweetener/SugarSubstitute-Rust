// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Complete installer surface copy resolved through the supported-locale registry.
use std::collections::BTreeMap;
mod translations;
const KEYS: &[&str] = &[
    "title",
    "close",
    "languageTitle",
    "languageDescription",
    "locationTitle",
    "locationDescription",
    "installFolder",
    "browse",
    "runtimeTitle",
    "managedTitle",
    "managedDescription",
    "attachedTitle",
    "attachedDescription",
    "channel",
    "version",
    "hardware",
    "mirrors",
    "comfyFolder",
    "python",
    "pythonHelp",
    "foldersTitle",
    "models",
    "outputs",
    "preservation",
    "progressTitle",
    "longWait",
    "details",
    "completeTitle",
    "back",
    "continue",
    "install",
    "open",
    "step",
    "noEnvironment",
    "retry",
    "cancelled",
    "cancelling",
    "workerStopped",
    "checkingSugarCubes",
    "installingSugarCubes",
    "sugarCubesReady",
    "setupFailed",
    "configurationTitleManaged",
    "configurationDescriptionManaged",
    "configurationTitleAttached",
    "configurationDescriptionAttached",
    "select",
    "selected",
    "progressDescription",
    "stepLanguage",
    "stepLocation",
    "stepRuntime",
    "stepConfiguration",
    "stepFolders",
    "stepProgress",
    "stepComplete",
    "hideDetails",
    "preparingInstallation",
    "stepMaintenance",
    "maintenanceTitle",
    "maintenanceDescription",
    "maintenanceMissing",
    "maintain",
    "maintenanceLaunchMissing",
    "recoveryConflict",
    "recoveryIncomplete",
    "recoveryInventoryBound",
];
/// Resolve installer labels with the same canonical fallback as every native surface.
pub fn messages(locale: &str) -> BTreeMap<&'static str, &'static str> {
    let values = match super::messages(locale).id {
        "zh-Hans" => translations::ZH_HANS,
        "ja" => translations::JA,
        "ko" => translations::KO,
        "es" => translations::ES,
        _ => translations::EN,
    };
    KEYS.iter().copied().zip(values.iter().copied()).collect()
}
#[cfg(test)]
mod tests {
    #[test]
    fn all_release_locales_have_every_installer_label() {
        for locale in super::super::LOCALES {
            let messages = super::messages(locale.id);
            assert_eq!(messages.len(), super::KEYS.len());
            assert!(messages.values().all(|value| !value.is_empty()));
            assert!(messages["step"].contains("{current}"));
            assert_ne!(messages["preparingInstallation"], messages["progressTitle"]);
        }
        assert_eq!(super::messages("unknown"), super::messages("en"));
    }
}
