// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Correlated application outcomes and presentation-only progress.
use super::*;

impl SetupFlow {
    /// Observe an outcome for the outstanding request, possibly admitting dependent work.
    /// # Errors
    /// Rejects stale identities, incompatible results and excessive text/options without
    /// consuming the pending request. A new request can fail if identities are exhausted.
    pub fn receive(
        &mut self,
        id: RequestId,
        outcome: Outcome,
        now: Duration,
    ) -> Result<Option<Request>, FlowError> {
        let pending = self
            .pending
            .as_ref()
            .filter(|pending| pending.id == id)
            .ok_or(FlowError::UnexpectedReply)?;
        let purpose = pending.purpose;
        validate(purpose, &outcome)?;
        let result = match outcome {
            Outcome::Completed(result) => result,
            Outcome::Failed(message) => {
                self.fail(Failure::Message(message));
                return Ok(None);
            }
            Outcome::Cancelled => {
                self.fail(Failure::Localized(
                    sugarsubstitute_contracts::localization::installer::InstallerText::Cancelled,
                ));
                return Ok(None);
            }
        };
        match (purpose, result) {
            (Purpose::Locale { advance }, Completed::Locale) => {
                self.copy_locale = self.choices.locale;
                self.pending = None;
                if advance {
                    self.show(Page::Location);
                }
            }
            (Purpose::Location, Completed::Location(location)) => {
                self.existing = location.existing;
                if self.existing.is_some() {
                    self.show(Page::Maintenance);
                } else {
                    self.choices.comfy_root = location.comfy_root;
                    self.choices.models = location.models;
                    self.choices.outputs = location.outputs;
                    self.show(Page::Runtime);
                }
            }
            (Purpose::Options(field), Completed::Options(options)) => {
                self.selected[field.index()] = options
                    .iter()
                    .rposition(|choice| choice.recommended)
                    .or(if options.is_empty() { None } else { Some(0) });
                self.options[field.index()] = options;
                match field {
                    OptionField::Release => {
                        return self.load_options(OptionField::ComfyVersion, now).map(Some);
                    }
                    OptionField::ComfyVersion => {
                        return self.load_options(OptionField::Variant, now).map(Some);
                    }
                    OptionField::Variant => {
                        self.pending = None;
                        self.options_valid = self.selected[field.index()].is_some();
                        if self.options_valid {
                            self.choices.selection_token = self
                                .selected_choice(field)
                                .and_then(|choice| choice.selection_token.clone());
                        } else {
                            self.failure = Some(Failure::Localized(sugarsubstitute_contracts::localization::installer::InstallerText::NoEnvironment));
                        }
                    }
                }
            }
            (Purpose::Runtime, Completed::Validated) => self.show(Page::Folders),
            (Purpose::Validate, Completed::Validated) => {
                self.begin_install(now);
                return self
                    .request(Purpose::Install, Operation::Install(self.choices.clone()), now)
                    .map(Some);
            }
            (Purpose::Install | Purpose::Maintain, Completed::Installed) => {
                self.overall = 100.0;
                self.show(Page::Complete);
            }
            (Purpose::Open, Completed::Opened) => {
                self.pending = None;
                self.close_requested = true;
            }
            _ => return Err(FlowError::UnexpectedReply),
        }
        Ok(None)
    }

    /// Observe correlated progress during installation. Non-finite values update only text.
    /// # Errors
    /// Rejects stale/non-installation progress and invalid status text without mutation.
    pub fn progress(
        &mut self,
        id: RequestId,
        status: String,
        overall: Option<f64>,
    ) -> Result<(), FlowError> {
        if !self.pending.as_ref().is_some_and(|pending| {
            pending.id == id && matches!(pending.purpose, Purpose::Install | Purpose::Maintain)
        }) {
            return Err(FlowError::UnexpectedReply);
        }
        valid_text(&status)?;
        self.status = Status::Message(status);
        if let Some(value) = overall.filter(|value| value.is_finite()) {
            self.overall = self.overall.max(value.clamp(0.0, 99.0));
        }
        Ok(())
    }

    /// Display worker loss and output; actual process/job retirement remains with the host.
    pub fn unavailable(&mut self) {
        if self.closing {
            return;
        }
        self.unavailable = true;
        self.retry = false;
        self.failure = Some(Failure::Localized(
            sugarsubstitute_contracts::localization::installer::InstallerText::WorkerStopped,
        ));
        self.show(Page::Progress);
        self.details = true;
    }

    /// Retain choices and expose Retry only after an installation attempt failed.
    fn fail(&mut self, failure: Failure) {
        self.pending = None;
        self.failure = Some(failure);
        self.retry = self.page == Page::Progress;
    }
}

/// Validate result kind and bounded retained data before consuming its request.
fn validate(purpose: Purpose, outcome: &Outcome) -> Result<(), FlowError> {
    let result = match outcome {
        Outcome::Failed(message) => return valid_text(message),
        Outcome::Cancelled => return Ok(()),
        Outcome::Completed(result) => result,
    };
    match (purpose, result) {
        (Purpose::Locale { .. }, Completed::Locale)
        | (Purpose::Runtime | Purpose::Validate, Completed::Validated)
        | (Purpose::Install | Purpose::Maintain, Completed::Installed)
        | (Purpose::Open, Completed::Opened) => Ok(()),
        (Purpose::Location, Completed::Location(location)) => {
            for text in [&location.comfy_root, &location.models, &location.outputs] {
                valid_text(text)?;
            }
            if let Some(existing) = &location.existing {
                valid_text(&existing.install_root)?;
                valid_text(&existing.comfy_root)?;
            }
            Ok(())
        }
        (Purpose::Options(_), Completed::Options(options)) => {
            if options.len() > 256 {
                return Err(FlowError::InvalidOptions);
            }
            let mut total = 0;
            for choice in options {
                for text in [&choice.value, &choice.label, &choice.description]
                    .into_iter()
                    .chain(choice.selection_token.iter())
                {
                    valid_text(text)?;
                    total += text.len();
                    if total > 65_536 {
                        return Err(FlowError::InvalidOptions);
                    }
                }
            }
            Ok(())
        }
        _ => Err(FlowError::UnexpectedReply),
    }
}
