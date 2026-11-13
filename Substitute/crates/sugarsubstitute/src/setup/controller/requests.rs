// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit explicit installer requests; rendering state grants no native authority.
use super::*;
mod input;
pub(in crate::setup) mod metadata;
use sugarsubstitute_application::setup_flow::{
    Choices, Operation as Command, OptionField, RuntimeMode,
};

/// One original request retained through package and worker preparation.
pub(super) struct Request {
    pub(super) id: String,
    pub(super) command: Command,
}
impl Request {
    /// Native maintenance and folder inspection never start an upstream worker.
    pub(super) fn needs_worker(&self) -> bool {
        matches!(&self.command, Command::Options { .. } | Command::Install(_))
            || matches!(&self.command, Command::Runtime(choices) if choices.mode == RuntimeMode::Attached)
    }
}

impl Controller {
    /// Admit typed application intent, retaining failures as replies to the original identity.
    pub(super) fn accept(&mut self, request: Request) -> Result<()> {
        let id = request.id.clone();
        if let Err(cause) = self.dispatch(request) {
            tracing::error!(operation="installer_request_failed", request_id=%id, cause_chain=%crate::diagnostics::CauseChain(cause.as_ref()), "Installer request failed");
            self.diagnostic(crate::failure_details::Details(cause.as_ref()).to_string())?;
            self.emit(Some(id), Kind::Failed(input::message(cause.as_ref(), &self.locale).into()))?;
        }
        Ok(())
    }
    /// Apply the same storage and provisioning admission regardless of the presenting surface.
    fn dispatch(&mut self, request: Request) -> Result<()> {
        if self.closing {
            return Err("Installer is closing".into());
        }
        if self.unavailable && !matches!(request.command, Command::Locale(_)) {
            return Err("Provisioning is unavailable; close and reopen setup".into());
        }
        let needs_worker = request.needs_worker();
        let needs_bundle = self.bundle.is_none()
            && (needs_worker || matches!(request.command, Command::Inspect(_)));
        if needs_bundle
            || (needs_worker && self.worker.as_ref().is_none_or(|worker| !worker.ready()))
        {
            if self.waiting.is_some() {
                return Err("Provisioning is still preparing".into());
            }
            if !needs_bundle {
                self.start_worker()?;
            }
            self.waiting = Some(request);
            return Ok(());
        }
        let Request { id, command } = request;
        match command {
            Command::Locale(locale) => {
                if !sugarsubstitute_contracts::localization::LOCALES
                    .iter()
                    .any(|entry| entry.id == locale)
                {
                    return Err("Unsupported locale".into());
                }
                self.locale = locale.to_owned();
                self.emit(Some(id), Kind::Completed(Completed::Locale))
            }
            Command::Inspect(root) => {
                let root = PathBuf::from(root);
                if !root.is_absolute() {
                    return Err(input::InputFailure::absolute().into());
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
            Command::Options { field, release, version, locale, mirrors } => {
                let field = match field {
                    OptionField::Release => "release",
                    OptionField::ComfyVersion => "comfyVersion",
                    OptionField::Variant => "variant",
                };
                let mut selections = serde_json::Map::new();
                if let Some(value) = release {
                    selections.insert("release".into(), json!(value));
                }
                if let Some(value) = version {
                    selections.insert("comfyVersion".into(), json!(value));
                }
                self.forward(id, "options", json!({"field":field,"selections":selections,"locale":locale,"mirrors":mirrors}), Purpose::Options)
            }
            command @ (Command::Runtime(_) | Command::Validate(_)) => {
                let runtime = matches!(command, Command::Runtime(_));
                let (Command::Runtime(choices) | Command::Validate(choices)) = command else {
                    return Err("Invalid validation command".into());
                };
                let selection = selection(choices);
                selection.validate().map_err(input::InputFailure::selection)?;
                if runtime && selection.mode == "attached" {
                    let mut input = json!({"directory":selection.comfy_root});
                    if !selection.python.is_empty() {
                        input["python"] = json!(selection.python);
                    }
                    self.forward(id, "inspect", input, Purpose::Inspect)
                } else {
                    self.emit(Some(id), Kind::Completed(Completed::Validated))
                }
            }
            command @ (Command::Install(_) | Command::Maintain(_)) => {
                if self.install_id.is_some() || self.pending.is_some() || self.installed.is_some() {
                    return Err("An installation transaction is already active".into());
                }
                let bundle =
                    Arc::clone(self.bundle.as_ref().ok_or("Installer package is not ready")?);
                let source = self.bundle_root.clone();
                let distribution = self.distribution;
                // A retry after the included-version sentence is the user's explicit choice.
                let offered = std::mem::take(&mut self.included_offered);
                let (maintenance_root, selection) = match command {
                    Command::Maintain(root) => (Some(PathBuf::from(root)), None),
                    Command::Install(choices) => {
                        let selection = selection(choices);
                        selection.validate().map_err(input::InputFailure::selection)?;
                        (None, Some(selection))
                    }
                    _ => return Err("Invalid installation command".into()),
                };
                self.selection = selection.clone();
                self.install_id = Some(id.clone());
                self.prepared = None;
                if let Err(cause) = self.submit(StorageWork::Import, move |context| {
                    let distribution = super::super::distribution::decode(distribution, &context)?;
                    let choice = crate::authority_clock::authorize(|now| {
                        metadata::select(&distribution, offered, now(), &context)
                    })?;
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
                        distribution.import_with(
                            session,
                            &source,
                            setup_storage::BundleSource::Retained(bundle),
                            choice,
                            now,
                            &context,
                        )
                    })
                    .map(|imported| StorageResult::Imported(Box::new(imported)))
                }) {
                    self.install_id = None;
                    return Err(cause);
                }
                self.emit(
                    Some(id),
                    Kind::Progress(Progress::storage(
                        sugarsubstitute_contracts::localization::installer::messages(&self.locale)
                            .preparing_installation
                            .into(),
                        0.0,
                    )),
                )
            }
            Command::Open => {
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
        }
    }
}

/// Storage receives a proposal and remains the sole validator of paths and provisioning authority.
pub(super) fn selection(value: Choices) -> Selection {
    Selection {
        locale: value.locale.into(),
        install_root: value.install_root.into(),
        mode: match value.mode {
            RuntimeMode::Managed => "managed",
            RuntimeMode::Attached => "attached",
        }
        .into(),
        comfy_root: value.comfy_root.into(),
        python: value.python,
        models: value.models.into(),
        outputs: value.outputs.into(),
        selection_token: value.selection_token,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_application_requests_preserve_identity_and_defer_inspection() -> Result<()> {
        use sugarsubstitute_application::setup_flow::{Completed, Outcome, SetupFlow};
        let mut flow = SetupFlow::new("E:\\用户\\Substitute".into())?;
        let mut controller = Controller::new(PathBuf::new(), PathBuf::new(), &[]);
        let request = flow.primary(std::time::Duration::ZERO)?.ok_or("Locale request absent")?;
        let id = request.id;
        controller.accept(Request { id: id.get().to_string(), command: request.operation })?;
        let reply = controller.output.pop_front().ok_or("Locale reply missing")?;
        assert_eq!(reply.id, Some(id.get().to_string()));
        assert!(matches!(reply.kind, Kind::Completed(super::Completed::Locale)));
        flow.receive(id, Outcome::Completed(Completed::Locale), std::time::Duration::ZERO)?;
        let request = flow.primary(std::time::Duration::ZERO)?.ok_or("Location request absent")?;
        let id = request.id.get().to_string();
        controller.accept(Request { id: id.clone(), command: request.operation })?;
        let retained = controller.waiting.as_ref().ok_or("Request not retained")?;
        assert_eq!(retained.command, Command::Inspect(flow.choices().install_root.clone()));
        assert_eq!(retained.id, id);
        assert!(!retained.needs_worker());
        assert!(controller.worker.is_none());
        controller.close()?;
        assert!(controller.waiting.is_none());
        Ok(())
    }
    #[test]
    fn option_commands_preserve_upstream_identities_and_mirror_choice() -> Result<()> {
        let command = Command::Options {
            field: OptionField::Variant,
            release: Some("release/id".into()),
            version: Some("版本/🐈".into()),
            locale: "ja",
            mirrors: true,
        };
        let expected = command.clone();
        let mut controller = Controller::new(PathBuf::new(), PathBuf::new(), &[]);
        controller.accept(Request { id: "options-2".into(), command })?;
        let pending = controller.waiting.as_ref().ok_or("Options request missing")?;
        assert!(pending.needs_worker());
        assert_eq!(pending.id, "options-2");
        assert_eq!(pending.command, expected);
        assert!(controller.worker.is_none());
        controller.close()?;
        Ok(())
    }
    #[test]
    fn rejected_native_command_retains_its_identity_without_starting_a_worker() -> Result<()> {
        let mut controller = Controller::new(PathBuf::new(), PathBuf::new(), &[]);
        for (command, expected) in [
            (Command::Locale("unknown"), "Unsupported locale"),
            (Command::Open, "Installation is not complete"),
        ] {
            controller.accept(Request { id: "original".into(), command })?;
            let diagnostic = controller.output.pop_front().ok_or("Original diagnostic absent")?;
            assert!(diagnostic.id.is_none());
            assert!(
                matches!(diagnostic.kind, Kind::Diagnostic(ref data) if data.message == expected && data.omitted == 0)
            );
            let response = controller.output.pop_front().ok_or("Failure not reported")?;
            assert_eq!(response.id.as_deref(), Some("original"));
            assert!(
                matches!(response.kind, Kind::Failed(ref message) if message == sugarsubstitute_contracts::localization::installer::messages("en").setup_failed)
            );
            assert!(controller.waiting.is_none() && controller.worker.is_none());
        }
        Ok(())
    }
    #[test]
    fn early_location_request_waits_for_package_verification_without_starting_provisioning()
    -> Result<()> {
        let mut controller = Controller::new(PathBuf::new(), PathBuf::new(), &[]);
        let request =
            |id: &str| Request { id: id.into(), command: Command::Inspect(r"E:\installed".into()) };
        controller.accept(request("first"))?;
        assert!(controller.output.is_empty());
        assert!(controller.worker.is_none());
        assert_eq!(controller.waiting.as_ref().ok_or("Request not retained")?.id, "first");
        controller.accept(request("second"))?;
        let diagnostic =
            controller.output.pop_front().ok_or("Competing request diagnostic absent")?;
        assert!(
            matches!(diagnostic.kind, Kind::Diagnostic(ref data) if data.message == "Provisioning is still preparing")
        );
        let rejection = controller.output.pop_front().ok_or("Competing request not rejected")?;
        assert_eq!(rejection.id.as_deref(), Some("second"));
        assert!(
            matches!(rejection.kind, Kind::Failed(ref message) if message == sugarsubstitute_contracts::localization::installer::messages("en").setup_failed)
        );
        assert_eq!(controller.waiting.as_ref().ok_or("First request replaced")?.id, "first");
        controller.close()?;
        assert!(controller.waiting.is_none() && controller.worker.is_none());
        Ok(())
    }
}
