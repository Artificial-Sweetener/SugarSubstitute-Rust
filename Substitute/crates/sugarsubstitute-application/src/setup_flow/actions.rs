// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admission of native installer input and dependent option discovery.
use super::*;
use sugarsubstitute_contracts::localization::LOCALES;

impl SetupFlow {
    /// Invoke the enabled primary action, returning work for the application adapter.
    /// # Errors
    /// Rejects unavailable actions or exhausted correlation identities.
    pub fn primary(&mut self, now: Duration) -> Result<Option<Request>, FlowError> {
        if !self.can_primary() {
            return Err(FlowError::ActionUnavailable);
        }
        self.failure = None;
        if self.retry {
            self.retry = false;
            self.show(if self.existing.is_some() {
                Page::Maintenance
            } else {
                Page::Configuration
            });
            return if self.existing.is_none() && self.choices.mode == RuntimeMode::Managed {
                self.load_options(OptionField::Release, now).map(Some)
            } else {
                Ok(None)
            };
        }
        let (purpose, operation) = match self.page {
            Page::Language => {
                (Purpose::Locale { advance: true }, Operation::Locale(self.choices.locale))
            }
            Page::Location => {
                (Purpose::Location, Operation::Inspect(self.choices.install_root.clone()))
            }
            Page::Runtime => {
                self.show(Page::Configuration);
                return if self.choices.mode == RuntimeMode::Managed {
                    self.load_options(OptionField::Release, now).map(Some)
                } else {
                    Ok(None)
                };
            }
            Page::Configuration => (Purpose::Runtime, Operation::Runtime(self.choices.clone())),
            Page::Folders => (Purpose::Validate, Operation::Validate(self.choices.clone())),
            Page::Maintenance => {
                let directory = self
                    .existing
                    .as_ref()
                    .ok_or(FlowError::ActionUnavailable)?
                    .install_root
                    .clone();
                self.begin_install(now);
                (Purpose::Maintain, Operation::Maintain(directory))
            }
            Page::Complete => (Purpose::Open, Operation::Open),
            Page::Progress => return Err(FlowError::ActionUnavailable),
        };
        self.request(purpose, operation, now).map(Some)
    }

    /// Return to the previous page while retaining field contents.
    /// # Errors
    /// Rejects navigation while work is outstanding or the page has no Back action.
    pub fn back(&mut self) -> Result<(), FlowError> {
        if !self.can_back() {
            return Err(FlowError::ActionUnavailable);
        }
        let previous = self
            .route()
            .windows(2)
            .find(|pair| pair[1] == self.page)
            .map(|pair| pair[0])
            .ok_or(FlowError::ActionUnavailable)?;
        self.failure = None;
        self.show(previous);
        Ok(())
    }

    /// Select a supported locale; displayed copy changes only after acceptance.
    /// # Errors
    /// Rejects unavailable input, unsupported locales or exhausted identities.
    pub fn set_locale(&mut self, locale: &str, now: Duration) -> Result<Request, FlowError> {
        self.input_on(Page::Language)?;
        let locale =
            LOCALES.iter().find(|entry| entry.id == locale).ok_or(FlowError::UnsupportedLocale)?.id;
        self.choices.locale = locale;
        self.request(Purpose::Locale { advance: false }, Operation::Locale(locale), now)
    }

    /// Retain an editable path verbatim; this performs no filesystem operation.
    /// # Errors
    /// Rejects input outside its editable page, during work, or with invalid text.
    pub fn set_path(&mut self, field: PathField, value: String) -> Result<(), FlowError> {
        let page = match field {
            PathField::Installation => Page::Location,
            PathField::Comfy | PathField::Python => {
                if self.choices.mode != RuntimeMode::Attached {
                    return Err(FlowError::ActionUnavailable);
                }
                Page::Configuration
            }
            PathField::Models | PathField::Outputs => Page::Folders,
        };
        self.input_on(page)?;
        valid_text(&value)?;
        *match field {
            PathField::Installation => &mut self.choices.install_root,
            PathField::Comfy => &mut self.choices.comfy_root,
            PathField::Python => &mut self.choices.python,
            PathField::Models => &mut self.choices.models,
            PathField::Outputs => &mut self.choices.outputs,
        } = value;
        Ok(())
    }

    /// Select runtime ownership without starting provisioning or erasing paths.
    /// # Errors
    /// Rejects unavailable runtime-page input.
    pub fn set_mode(&mut self, mode: RuntimeMode) -> Result<(), FlowError> {
        self.input_on(Page::Runtime)?;
        self.choices.mode = mode;
        Ok(())
    }

    /// Reload upstream options after changing the mirror preference.
    /// # Errors
    /// Rejects unavailable managed input or exhausted identities.
    pub fn set_mirrors(&mut self, enabled: bool, now: Duration) -> Result<Request, FlowError> {
        self.managed_input()?;
        self.mirrors = enabled;
        self.failure = None;
        self.load_options(OptionField::Release, now)
    }

    /// Select an upstream option and request its dependent list when necessary.
    /// # Errors
    /// Rejects unavailable input, an invalid index or exhausted identities.
    pub fn select_option(
        &mut self,
        field: OptionField,
        index: usize,
        now: Duration,
    ) -> Result<Option<Request>, FlowError> {
        self.managed_input()?;
        if index >= self.options[field.index()].len() {
            return Err(FlowError::InvalidOptions);
        }
        self.selected[field.index()] = Some(index);
        match field {
            OptionField::Release | OptionField::ComfyVersion => {
                self.failure = None;
                self.load_options(
                    if field == OptionField::Release {
                        OptionField::ComfyVersion
                    } else {
                        OptionField::Variant
                    },
                    now,
                )
                .map(Some)
            }
            OptionField::Variant => {
                self.choices.selection_token =
                    self.selected_choice(field).and_then(|choice| choice.selection_token.clone());
                Ok(None)
            }
        }
    }

    /// Current page fields may accept input only while the application can admit their changes.
    pub fn can_edit(&self) -> bool {
        !self.busy() && !self.closing && !self.unavailable && !self.close_requested
    }
    /// Gate edits against both page visibility and outstanding application work.
    fn input_on(&self, page: Page) -> Result<(), FlowError> {
        if self.page != page || !self.can_edit() {
            Err(FlowError::ActionUnavailable)
        } else {
            Ok(())
        }
    }
    /// Restrict upstream discovery controls to the selected managed-runtime page.
    fn managed_input(&self) -> Result<(), FlowError> {
        self.input_on(Page::Configuration)?;
        if self.choices.mode != RuntimeMode::Managed {
            Err(FlowError::ActionUnavailable)
        } else {
            Ok(())
        }
    }
    /// Invalidate the old provisioning token before requesting its replacement list.
    pub(super) fn load_options(
        &mut self,
        field: OptionField,
        now: Duration,
    ) -> Result<Request, FlowError> {
        self.options_valid = false;
        self.choices.selection_token = None;
        let operation = Operation::Options {
            field,
            release: self.selected_choice(OptionField::Release).map(|choice| choice.value.clone()),
            version: self
                .selected_choice(OptionField::ComfyVersion)
                .map(|choice| choice.value.clone()),
            locale: self.choices.locale,
            mirrors: self.mirrors,
        };
        self.request(Purpose::Options(field), operation, now)
    }
    /// Reset progress only when starting a newly admitted installation attempt.
    pub(super) fn begin_install(&mut self, now: Duration) {
        self.show(Page::Progress);
        self.started = now;
        self.overall = 0.0;
        self.status = Status::Preparing;
    }
}
