// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit explicit installer requests; rendering state grants no native authority.
use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u32,
    id: String,
    operation: String,
    input: Value,
}

impl Controller {
    pub(super) fn request_value(&mut self, text: &str) -> Result<()> {
        if text.len() > 65536 {
            return Err("Installer request is too large".into());
        }
        let request: Request = serde_json::from_str(text)?;
        if request.version != 1
            || request.id.len() > 80
            || request.id.is_empty()
            || !request.id.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            return Err("Invalid installer request identity".into());
        }
        let id = request.id.clone();
        if let Err(cause) = self.dispatch(request) {
            self.reply(Some(&id), "failed", json!({"message":cause.to_string()}))?;
        }
        Ok(())
    }
    fn dispatch(&mut self, request: Request) -> Result<()> {
        if self.closing {
            return Err("Installer is closing".into());
        }
        if self.unavailable && request.operation != "locale" {
            return Err("Provisioning is unavailable; close and reopen setup".into());
        }
        let Request { id, operation, input, .. } = request;
        let needs_worker = matches!(operation.as_str(), "options" | "install")
            || (operation == "runtime" && input["mode"] == "attached");
        let needs_bundle = self.bundle.is_none() && (needs_worker || operation == "location");
        if needs_bundle
            || (needs_worker && self.worker.as_ref().is_none_or(|worker| !worker.ready()))
        {
            if self.waiting.is_some() {
                return Err("Provisioning is still preparing".into());
            }
            if !needs_bundle {
                self.start_worker()?;
            }
            self.waiting = Some(json!({"version":1,"id":id,"operation":operation,"input":input}));
            return Ok(());
        }
        match operation.as_str() {
            "locale" => {
                let locale = input["locale"].as_str().ok_or("Locale missing")?;
                if !sugarsubstitute_contracts::localization::LOCALES
                    .iter()
                    .any(|entry| entry.id == locale)
                {
                    return Err("Unsupported locale".into());
                }
                self.locale = locale.to_owned();
                self.reply(
                    Some(&id),
                    "completed",
                    json!(sugarsubstitute_contracts::localization::installer::messages(locale)),
                )
            }
            "location" => {
                let root = PathBuf::from(
                    input["directory"].as_str().ok_or("Installation folder missing")?,
                );
                if !root.is_absolute() {
                    return Err("Choose an absolute installation folder".into());
                }
                if self.inspection_id.is_some() || self.install_id.is_some() {
                    return Err("An installation operation is already active".into());
                }
                self.inspection_id = Some(id);
                if let Err(cause) = self.submit(StorageWork::Metadata, move |context| {
                    let existing = setup_storage::inspect_installation(&root, &context)?;
                    Ok(StorageResult::Inspected(root, existing.map(Box::new)))
                }) {
                    self.inspection_id = None;
                    return Err(cause);
                }
                Ok(())
            }
            "options" => self.forward(id, "options", input, Purpose::Options),
            "runtime" | "validate" => {
                let selection: Selection = serde_json::from_value(input)?;
                selection.validate()?;
                if operation == "runtime" && selection.mode == "attached" {
                    let mut input = json!({"directory":selection.comfy_root});
                    if !selection.python.is_empty() {
                        input["python"] = json!(selection.python);
                    }
                    self.forward(id, "inspect", input, Purpose::Inspect)
                } else {
                    self.reply(Some(&id), "completed", json!({}))
                }
            }
            "install" | "maintain" => {
                if self.install_id.is_some() || self.pending.is_some() || self.installed.is_some() {
                    return Err("An installation transaction is already active".into());
                }
                let bundle =
                    Arc::clone(self.bundle.as_ref().ok_or("Installer package is not ready")?);
                let source = self.bundle_root.clone();
                let distribution = self.distribution;
                let maintenance_root = if operation == "maintain" {
                    Some(PathBuf::from(
                        input["directory"].as_str().ok_or("Installation folder missing")?,
                    ))
                } else {
                    None
                };
                let selection = if maintenance_root.is_none() {
                    let selection: Selection = serde_json::from_value(input)?;
                    selection.validate()?;
                    Some(selection)
                } else {
                    None
                };
                self.selection = selection.clone();
                self.install_id = Some(id.clone());
                self.prepared = None;
                if let Err(cause) = self.submit(StorageWork::Import, move |context| {
                    let distribution = super::super::distribution::decode(distribution, &context)?;
                    let session = if let Some(root) = maintenance_root {
                        setup_storage::InstallationSession::maintain(&root, &context)?
                    } else {
                        setup_storage::InstallationSession::create(
                            selection
                                .ok_or_else(|| io::Error::other("Installation choices absent"))?,
                            &context,
                        )?
                    };
                    crate::authority_clock::authorize(|now| {
                        distribution.import(
                            session,
                            &source,
                            setup_storage::BundleSource::Retained(bundle),
                            now,
                            &context,
                        )
                    })
                    .map(|imported| StorageResult::Imported(Box::new(imported)))
                }) {
                    self.install_id = None;
                    return Err(cause);
                }
                self.reply(Some(&id), "progress", json!({"overall":0,"status":sugarsubstitute_contracts::localization::installer::messages(&self.locale)["preparingInstallation"]}))
            }
            "open" => {
                if self.install_id.is_some() || self.storage.is_some() || self.relaunch.is_some() {
                    return Err("An application launch is already active".into());
                }
                let installed =
                    self.installed.as_ref().ok_or("Installation is not complete")?.clone();
                self.install_id = Some(id);
                if let Err(cause) = self.open_installation(installed) {
                    self.install_id = None;
                    return Err(cause);
                }
                Ok(())
            }
            _ => Err("Unsupported installer operation".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_desktop::installer::InstallerLifetime;

    #[test]
    fn early_location_request_waits_for_package_verification_without_starting_provisioning()
    -> Result<()> {
        let mut controller =
            Controller::new(PathBuf::from(r"E:\bundle"), PathBuf::from(r"E:\work"), &[]);
        let request = |id| {
            json!({"version":1,"id":id,"operation":"location",
            "input":{"directory":r"E:\installed"}})
            .to_string()
        };
        controller.request_value(&request("first"))?;
        assert!(controller.output.is_empty());
        assert!(controller.worker.is_none());
        assert_eq!(controller.waiting.as_ref().ok_or("Request was not retained")?["id"], "first");
        controller.request_value(&request("second"))?;
        let rejection =
            controller.output.pop_front().ok_or("Competing request was not rejected")?;
        assert_eq!(rejection["id"], "second");
        assert_eq!(rejection["event"], "failed");
        assert_eq!(controller.waiting.as_ref().ok_or("First request was replaced")?["id"], "first");
        controller.close()?;
        assert!(controller.waiting.is_none());
        assert!(controller.worker.is_none());
        Ok(())
    }
}
