// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Permit only an already-running helper to finish briefly after application closure.
use super::*;
use std::time::Duration;

const FINISH_ALLOWANCE: Duration = Duration::from_secs(120);

/// Immutable cutoffs in the original executor's epoch; delayed observations never renew them.
pub(super) struct Window {
    pub(super) finish_until: Moment,
    retirement: ShutdownBudget,
}
impl Window {
    /// Preserve the campaign deadline and cap continuation from the original application close.
    fn plan(application: ShutdownBudget, campaign: Moment, now: Moment) -> Result<Option<Self>> {
        application.validate()?;
        let finish_until = campaign.min(application.started.checked_add(FINISH_ALLOWANCE)?);
        if now >= finish_until {
            return Ok(None);
        }
        Ok(Some(Self { finish_until, retirement: ShutdownBudget::new(finish_until)? }))
    }
}

impl Parent {
    /// A normal user close may let an existing contained helper finish for up to two minutes,
    /// followed by the fixed native cleanup interval. No new helper starts after this decision.
    /// The coordinator must release all application custody before entering that continuation.
    /// Failed startup, partial creation, borrowed run guards and unexpected shutdown close normally.
    pub(crate) fn application_closing(
        &mut self,
        application: Option<ShutdownBudget>,
        normal: bool,
    ) -> Result<()> {
        if !normal {
            return self.close(application);
        }
        if self.shutdown.is_some() || self.retired || self.continuation.is_some() {
            return Ok(());
        }
        let started = self
            .process
            .and_then(|process| {
                self.retained
                    .as_ref()
                    .and_then(|held| held.registry.as_ref())
                    .map(|registry| registry.snapshot(process))
            })
            .transpose()?
            .is_some_and(|snapshot| {
                snapshot.started
                    && snapshot.root_exit.is_none()
                    && !snapshot.failed
                    && !snapshot.launch_closed
            });
        if !started || self.failure.is_some() || self.borrows_application() {
            return self.close(application);
        }
        let Some(application) = application else { return self.close(None) };
        let now = self.client.now()?;
        let Some(window) = Window::plan(application, self.until, now)? else {
            return self.close(Some(application));
        };
        let remaining = window.finish_until.checked_duration_since(now)?;
        self.until = window.finish_until;
        self.wake_at =
            Instant::now().checked_add(remaining).ok_or("Post-exit update deadline overflow")?;
        self.continuation = Some(window);
        Ok(())
    }

    /// The final helper cutoff can tighten on early exit/failure, but cannot exceed its fixed window.
    /// This grants no application-release proof; the coordinator independently establishes that.
    pub(crate) fn post_exit_limit(&self) -> Option<Moment> {
        let window = self.continuation.as_ref()?;
        Some(self.shutdown.map_or(window.retirement.settle_until, |budget| {
            budget.settle_until.min(window.retirement.settle_until)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn seconds(value: u64) -> Moment {
        Moment::from_nanos(value * 1_000_000_000)
    }

    #[test]
    fn continuation_caps_work_from_original_close_and_keeps_separate_cleanup() -> Result<()> {
        let app = ShutdownBudget::new(seconds(50))?;
        let window = Window::plan(app, seconds(900), seconds(60))?.ok_or("Window absent")?;
        assert_eq!(window.finish_until, seconds(170));
        assert_eq!(window.retirement.settle_until, seconds(185));
        assert_eq!(app.settle_until, seconds(65));
        Ok(())
    }

    #[test]
    fn continuation_never_extends_the_existing_campaign() -> Result<()> {
        let app = ShutdownBudget::new(seconds(50))?;
        let window = Window::plan(app, seconds(80), seconds(60))?.ok_or("Window absent")?;
        assert_eq!(window.finish_until, seconds(80));
        assert_eq!(window.retirement.settle_until, seconds(95));
        assert!(Window::plan(app, seconds(80), seconds(80))?.is_none());
        assert!(Window::plan(app, seconds(900), seconds(170))?.is_none());
        Ok(())
    }

    #[test]
    fn invalid_application_budget_does_not_create_a_continuation() -> Result<()> {
        let mut app = ShutdownBudget::new(seconds(50))?;
        app.settle_until = seconds(500);
        assert!(Window::plan(app, seconds(900), seconds(60)).is_err());
        Ok(())
    }
}
