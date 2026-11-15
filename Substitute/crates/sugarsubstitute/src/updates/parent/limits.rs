// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Limit the two waits of a preparation helper that its own deadlines cannot end: the check
//! before any download, and leaving after its result. A download and the preparation after it
//! have no overall limit; their step deadlines and the transport's stall timeout bound them.
use super::*;
use std::time::Duration;

/// Longest a helper may take to start, open update storage and read the repository's metadata.
/// Every network phase already ends at its stall timeout; this bounds the check as a whole, so a
/// server that keeps answering too slowly, or a wait the helper itself cannot end, becomes a
/// failed check that the schedule retries.
pub(in crate::updates) const CHECK: Duration = Duration::from_secs(120);
/// Longest a helper may keep running after it delivered its result.
pub(in crate::updates) const LEAVE: Duration = Duration::from_secs(30);

/// A wait of an open helper that ends at a limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Wait {
    /// The helper has reached neither a download nor a result.
    Check,
    /// The helper delivered its result and has not exited.
    Leave,
}
impl Wait {
    /// The limited wait an open helper is in, from what its attempt channel last showed. A helper
    /// that has shown nothing yet is still starting, which is part of the check.
    pub(super) fn of(phase: Option<AttemptPhase>, has_outcome: bool) -> Option<Self> {
        if has_outcome {
            return Some(Self::Leave);
        }
        matches!(phase, None | Some(AttemptPhase::Opening | AttemptPhase::Checking))
            .then_some(Self::Check)
    }

    pub(super) const fn limit(self) -> Duration {
        match self {
            Self::Check => CHECK,
            Self::Leave => LEAVE,
        }
    }
}

impl Parent {
    /// Start the limit of a wait the helper entered, drop the limit of a wait it left, and end a
    /// wait that reached its limit: a check fails, and a helper that will not leave is closed
    /// with the result it delivered. A probe and a helper finishing after the application closed
    /// keep their own outer cutoff.
    /// # Errors
    /// Returns the failed check for the caller to record; its helper is already being stopped,
    /// without a grace interval.
    pub(super) fn observe_limit(&mut self, now: Moment) -> Result<()> {
        if self.probe
            || self.continuation.is_some()
            || self.shutdown.is_some()
            || self.exit.is_some()
        {
            return Ok(());
        }
        let (phase, has_outcome) = self.attempt.as_ref().map_or((None, false), |channel| {
            (channel.progress().map(|progress| progress.phase), channel.has_outcome())
        });
        let wait = Wait::of(phase, has_outcome);
        if self.limit.map(|(held, _)| held) != wait {
            self.limit = match wait {
                Some(wait) => Some((wait, now.checked_add(wait.limit())?)),
                None => None,
            };
            self.wake_at = wait.and_then(|wait| Instant::now().checked_add(wait.limit()));
            return Ok(());
        }
        let Some((wait, _)) = self.limit.filter(|(_, until)| now >= *until) else {
            return Ok(());
        };
        self.limit = None;
        self.wake_at = None;
        match wait {
            Wait::Check => {
                // A check has written nothing a stop could cut short, so the helper is stopped
                // now instead of after a close's grace: the failed check is reported at the
                // limit, not seconds after it.
                self.close(Some(ShutdownBudget {
                    graceful_until: now,
                    ..ShutdownBudget::new(now)?
                }))?;
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    if phase == Some(AttemptPhase::Checking) {
                        format!(
                            "The update server did not answer within {} seconds",
                            CHECK.as_secs()
                        )
                    } else {
                        format!(
                            "Update preparation did not start checking within {} seconds",
                            CHECK.as_secs()
                        )
                    },
                )
                .into())
            }
            Wait::Leave => {
                tracing::warn!(
                    operation = "update_helper_overstayed",
                    "The update helper delivered its result and did not exit; closing it"
                );
                self.close(None)
            }
        }
    }
}
