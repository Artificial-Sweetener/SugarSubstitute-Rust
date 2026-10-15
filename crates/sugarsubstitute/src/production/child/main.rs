// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the actual grant and finite main-request construction through native profile authorization.
use super::*;
use configuration::PreparedMainInput;
use profile_transaction::ProfileTransaction;
use sugarsubstitute_contracts::configuration::LoopbackOrigin;
#[path = "main/worker.rs"]
mod worker;
use worker::PreparedMain;
#[path = "main/failure.rs"]
mod failure;
use failure::MainPreparationFailure;
#[path = "main/terminal.rs"]
mod terminal;
use terminal::{MainTerminalIssue, MainTerminalOwner};
type MainJob = OwnedJob<PreparedMain, MainPreparationFailure>;
type MainTerminal = JobCompletion<PreparedMain, MainPreparationFailure>;
pub(super) struct MainPreparation {
    pub(in super::super) settings: MainWindowSettings,
    input: Option<worker::MainConstruction>,
    job: Option<MainJob>,
    noticed: Option<TerminalKind>,
    unshared: Option<MainTerminal>,
    terminal: Option<MainTerminalOwner>,
    pub(super) request_consumed: bool,
}
impl MainPreparation {
    pub(super) fn new(settings: MainWindowSettings) -> Self {
        Self {
            settings,
            input: None,
            job: None,
            noticed: None,
            unshared: None,
            terminal: None,
            request_consumed: false,
        }
    }
    pub(super) fn capture(&mut self, batch: &CompletionBatch) {
        if let Some(job) = &self.job {
            for notice in &batch.notifications {
                if (notice.scope_id, notice.job_id) == (job.scope_id(), job.spec().id) {
                    self.noticed = Some(notice.kind)
                }
            }
        }
    }
    pub(super) fn deadline(&self) -> Option<Moment> {
        self.job.as_ref().and_then(|job| job.spec().deadline).or_else(|| {
            self.profile()
                .filter(|profile| !profile.authorized())
                .map(ProfileTransaction::original_deadline)
        })
    }
    pub(super) fn profile(&self) -> Option<&ProfileTransaction> {
        match &self.terminal.as_ref()?.get()?.outcome {
            JobOutcome::Success(value) => Some(&value.profile),
            _ => None,
        }
    }
    pub(super) fn profile_mut(&mut self) -> Option<&mut ProfileTransaction> {
        match &mut self.terminal.as_mut()?.get_mut()?.outcome {
            JobOutcome::Success(value) => Some(&mut value.profile),
            _ => None,
        }
    }
    pub(in super::super) fn take_reusable_input(&mut self) -> Result<Option<PreparedMainInput>> {
        if self.pending() {
            return Err("Original main worker is not settled".into());
        }
        if self.input.is_some() {
            return Err("Original main construction remains retained".into());
        }
        if let Some(owner) = &mut self.terminal {
            let completion = owner.get_mut().ok_or("Original main outcome remains aliased")?;
            if let JobOutcome::Success(value) = &mut completion.outcome {
                return Ok(value.reusable_input.take());
            }
        }
        Ok(None)
    }
    pub(in super::super) fn restore_reusable_input(
        &mut self,
        input: &mut Option<PreparedMainInput>,
    ) -> Result<()> {
        let completion = self
            .terminal
            .as_mut()
            .and_then(MainTerminalOwner::get_mut)
            .ok_or("Original main completion absent")?;
        if let JobOutcome::Success(value) = &mut completion.outcome {
            if value.reusable_input.is_some() {
                return Err("Reusable input replacement rejected".into());
            }
            value.reusable_input = input.take();
            return Ok(());
        }
        Err("Original reusable main input lost its successful owner".into())
    }
    /// A native absence check also excludes original failed-admission input custody.
    pub(super) fn construction_pending(&self) -> bool {
        self.pending()
            || self.input.as_ref().is_some_and(|input| input.frame.is_some())
            || self.terminal.as_ref().is_some_and(|owner| {
                match owner.get().map(|terminal| &terminal.outcome) {
                    Some(JobOutcome::Success(value)) => value.frame.is_some(),
                    Some(JobOutcome::Failure(value)) => value.frame_pending(),
                    Some(_) => false,
                    None => true,
                }
            })
    }
    pub(super) fn pending(&self) -> bool {
        self.job.is_some() || self.unshared.is_some()
    }
    pub(super) fn close(&mut self) {
        if let Some(job) = &self.job {
            job.cancel()
        }
        if let Some(profile) = self.profile_mut() {
            profile.close()
        }
    }
    pub(super) fn release_frame(&mut self) -> Result<()> {
        if self.pending() {
            return Err("Original Main constructor has not retired".into());
        }
        if let Some(input) = &mut self.input {
            input.frame = None;
        }
        if let Some(terminal) = &mut self.terminal {
            let terminal = terminal.get().ok_or("Original main terminal absent")?;
            match &terminal.outcome {
                JobOutcome::Success(_) => {
                    let done =
                        self.terminal.as_mut().and_then(MainTerminalOwner::get_mut).ok_or(
                            "Successful main terminal unexpectedly aliased during retirement",
                        )?;
                    if let JobOutcome::Success(value) = &mut done.outcome {
                        value.frame = None;
                    }
                }
                JobOutcome::Failure(value) => value.release_frame()?,
                _ => {}
            }
        }
        Ok(())
    }
}
impl ProductionChild {
    pub(super) fn begin_main(
        &mut self,
        prepared: &mut PreparedChildStartup,
        origin: LoopbackOrigin,
    ) -> Result<()> {
        if self.frontend_budget.is_some() || self.main.pending() || self.main.terminal.is_some() {
            return Err("Main construction duplicated".into());
        }
        let budget = FrontendBudget::new(self.client()?.now()?)?;
        self.frontend_budget = Some(budget);
        let header = self.grant_header.ok_or("Original grant request absent")?;
        let client = self.client()?.clone();
        let id = self.next_job()?;
        let scope = self.bootstrap.scope().ok_or("Actual child finite scope absent")?;
        if self.grant.is_none() {
            return Err("Actual authenticated grant frame absent".into());
        }
        let input =
            prepared.take_main_input().ok_or("Original guarded main input already transferred")?;
        let settings = requested_settings(
            self.main.settings,
            prepared.session().configuration().presentation(),
        );
        self.main.input = Some(worker::MainConstruction {
            frame: self.grant.take(),
            input,
            identity: self.identity,
            header,
            origin,
            settings,
            original: budget.until(),
        });
        self.main.job = Some(worker::submit(&client, scope, id, &mut self.main.input)?);
        Ok(())
    }
    pub(super) fn consume_main(&mut self) -> Result<()> {
        if self.main.terminal.is_none() && self.main.noticed.is_some() {
            let Some(job) = &mut self.main.job else {
                return Err("Main completion job absent".into());
            };
            let Some(completion) = job.try_take_completion() else { return Ok(()) };
            let matches = completion.scope_id == job.scope_id()
                && completion.spec == job.spec()
                && Some(completion.outcome.kind()) == self.main.noticed;
            self.main.unshared = Some(completion);
            self.main.job = None;
            self.main.noticed = None;
            if !matches {
                return Err("Main completion original metadata differs".into());
            }
        }
        if self.main.unshared.is_some() {
            self.main.terminal = Some(MainTerminalOwner::new(&mut self.main.unshared)?);
        }
        let Some(owner) = &mut self.main.terminal else { return Ok(()) };
        let terminal = owner.get().ok_or("Actual main terminal absent")?;
        if terminal.outcome.kind() != TerminalKind::Success {
            return if self.shutdown.closing() {
                Ok(())
            } else {
                Err(Box::new(MainTerminalIssue(owner.clone())))
            };
        }
        let terminal =
            owner.get_mut().ok_or("Actual successful main terminal unexpectedly aliased")?;
        if let JobOutcome::Success(value) = &mut terminal.outcome {
            if !self.main.request_consumed {
                if self.shutdown.closing() {
                    value.profile.close();
                    value.frame = None;
                    return Ok(());
                }
                self.bootstrap.channel_mut().consume_reply(value.header)?;
                self.main.request_consumed = true;
                value.frame = None;
            }
            Ok(())
        } else {
            Err("Actual main terminal classification changed".into())
        }
    }
    pub(super) fn advance_profile(&mut self) -> Result<()> {
        if self.commands.iter().all(Option::is_some) {
            return Err("Desktop command slots full before profile reply transfer".into());
        }
        let Some(profile) = self.main.profile_mut() else { return Ok(()) };
        let channel = self.bootstrap.channel_mut();
        if profile.awaiting_reply()
            && channel.received_header()?.is_some_and(|header| profile.expects_reply(header))
        {
            match profile.take_reply(channel) {
                Ok(Some(command)) => self.push_command(command)?,
                Ok(None) => {}
                Err(rejected) => {
                    self.failed_frame = Some(rejected.frame);
                    return Err(rejected.error);
                }
            }
        }
        if let Some(profile) = self.main.profile_mut() {
            profile.activity(self.bootstrap.channel_mut())?;
        }
        Ok(())
    }
}

fn requested_settings(
    mut settings: MainWindowSettings,
    presentation: sugarsubstitute_contracts::configuration::Presentation,
) -> MainWindowSettings {
    let copy = sugarsubstitute_contracts::localization::messages(presentation.locale().as_str());
    let labels = copy.window;
    settings.recovery = sugarsubstitute_desktop::MainRecoveryLabels {
        retry: copy.retry,
        retry_ready: copy.retry_ready,
    };
    settings.dark = presentation.theme() == sugarsubstitute_contracts::configuration::Theme::Dark;
    settings.labels = sugarsubstitute_desktop::MainCaptionLabels {
        minimize: labels.minimize,
        maximize: labels.maximize,
        restore: labels.restore,
        close: labels.close,
    };
    settings
}

#[cfg(test)]
mod settings_tests {
    use super::*;
    use sugarsubstitute_contracts::configuration::{Presentation, Theme};
    #[test]
    fn authenticated_presentation_controls_caption_and_theme_without_changing_geometry()
    -> Result<()> {
        for locale in sugarsubstitute_contracts::localization::LOCALES {
            for theme in [Theme::Light, Theme::Dark] {
                let settings = requested_settings(
                    MainWindowSettings {
                        recovery: sugarsubstitute_desktop::MainRecoveryLabels {
                            retry: sugarsubstitute_contracts::localization::messages("en").retry,
                            retry_ready: sugarsubstitute_contracts::localization::messages("en")
                                .retry_ready,
                        },
                        logical_width: 1280,
                        logical_height: 720,
                        dark: true,
                        labels: sugarsubstitute_desktop::MainCaptionLabels {
                            minimize: "initial",
                            maximize: "initial",
                            restore: "initial",
                            close: "initial",
                        },
                    },
                    Presentation::new(locale.id, theme, false)?,
                );
                let copy = sugarsubstitute_contracts::localization::messages(locale.id);
                let expected = copy.window;
                assert_eq!(
                    (settings.recovery.retry, settings.recovery.retry_ready),
                    (copy.retry, copy.retry_ready),
                );
                assert_eq!((settings.logical_width, settings.logical_height), (1280, 720));
                assert_eq!(settings.dark, theme == Theme::Dark);
                assert_eq!(
                    [
                        settings.labels.minimize,
                        settings.labels.maximize,
                        settings.labels.restore,
                        settings.labels.close
                    ],
                    [expected.minimize, expected.maximize, expected.restore, expected.close]
                );
            }
        }
        Ok(())
    }
}
