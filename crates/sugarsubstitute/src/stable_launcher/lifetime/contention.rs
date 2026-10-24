// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retry short selection writers through native event-loop wakes and the original startup budget.
use super::*;
use std::time::Duration;
use sugarsubstitute_supervision::setup_storage::SelectionFailure;

/// A pending read holds no file, run lease or execution job while another writer completes.
pub(super) struct Wait {
    after: Moment,
    pub(super) wake_at: Instant,
    pub(super) cause: Box<dyn Error>,
}
impl Wait {
    /// Cap each paced observation at the original cutoff; retries cannot renew startup time.
    pub(super) fn new(now: Moment, until: Moment, cause: Box<dyn Error>) -> Result<Self> {
        if now >= until {
            return Err(cause);
        }
        let after = now.checked_add(Duration::from_millis(100))?.min(until);
        let wake_at = Instant::now()
            .checked_add(after.checked_duration_since(now)?)
            .ok_or("Selection wake overflow")?;
        Ok(Self { after, wake_at, cause })
    }
}

/// Retry only the selection owner's typed contention. Other locks, corrupt state, cancellation
/// and failed protection retirement remain errors even when their source includes contention.
pub(super) fn writer_busy(mut cause: &(dyn Error + 'static)) -> bool {
    loop {
        if let Some(selection) = cause.downcast_ref::<SelectionFailure>() {
            match selection {
                SelectionFailure::WriterBusy => return true,
                SelectionFailure::Storage(storage) => cause = storage,
                _ => return false,
            }
        } else if let Some(storage) = cause.downcast_ref::<io::Error>() {
            let Some(inner) = storage.get_ref() else { return false };
            cause = inner;
        } else {
            return false;
        }
    }
}

impl Launcher {
    /// Early native/completion wakes do not admit another read or busy-spin the UI thread.
    pub(super) fn selection_due(&mut self) -> bool {
        if self.selection_wait.as_ref().is_some_and(|wait| self.clock.now() < wait.after) {
            return false;
        }
        self.selection_wait = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{finish, fixture};
    use super::*;
    use crate::updates::test_support::observe;
    use std::sync::atomic::Ordering;

    #[test]
    fn only_typed_writer_contention_is_retryable() {
        assert!(writer_busy(&io::Error::other(SelectionFailure::WriterBusy)));
        assert!(writer_busy(&io::Error::other(SelectionFailure::Storage(io::Error::other(
            SelectionFailure::WriterBusy,
        )))));
        assert!(!writer_busy(&io::Error::from(io::ErrorKind::WouldBlock)));
        assert!(!writer_busy(&SelectionFailure::Retirement {
            operation: Box::new(SelectionFailure::WriterBusy),
            retirement: io::Error::other("Native retirement failed"),
        }));
        assert!(!writer_busy(&SelectionFailure::Changed));
    }

    #[test]
    fn writer_contention_waits_without_jobs_and_preserves_startup_cutoff() -> Result<()> {
        let (mut launcher, clock, wake) = fixture()?;
        let cutoff = launcher.until;
        launcher
            .failed(Task::Installation, io::Error::other(SelectionFailure::WriterBusy).into())?;
        let first = launcher.next_deadline();
        assert!(matches!(launcher.choice, Choice::Installation));
        for _ in 0..4 {
            launcher.select_next()?;
            assert!(launcher.job.is_none());
            assert_eq!(launcher.next_deadline(), first);
        }
        assert!(launcher.update_failure.is_none());
        clock.0.store(100_000_000, Ordering::SeqCst);
        assert!(launcher.selection_due());
        assert!(launcher.selection_wait.is_none());
        launcher.choice = Choice::Accepted;
        launcher.failed(Task::Select, SelectionFailure::WriterBusy.into())?;
        assert!(matches!(launcher.choice, Choice::Accepted));
        assert_eq!(launcher.until, cutoff);
        launcher.request_shutdown()?;
        assert!(launcher.selection_wait.is_none());
        finish(&mut launcher, &wake)
    }

    #[test]
    fn writer_contention_never_renews_expired_startup_or_retries_after_close() -> Result<()> {
        let (mut launcher, clock, wake) = fixture()?;
        let until = launcher.until.ok_or("Cutoff absent")?;
        clock.0.store(until.as_nanos(), Ordering::SeqCst);
        assert!(launcher.failed(Task::Select, SelectionFailure::WriterBusy.into()).is_err());
        assert!(launcher.selection_wait.is_none());
        launcher.request_shutdown()?;
        assert!(launcher.failed(Task::Installation, SelectionFailure::WriterBusy.into()).is_err());
        finish(&mut launcher, &wake)
    }

    #[test]
    fn actual_worker_contention_retires_before_waiting_for_the_next_selection() -> Result<()> {
        let (mut launcher, _, wake) = fixture()?;
        launcher.choice = Choice::Accepted;
        launcher.submit(Task::Select, |_| Err(io::Error::other(SelectionFailure::WriterBusy)))?;
        observe(&wake, || {
            launcher.advance(NonZeroUsize::MIN)?;
            Ok(launcher.selection_wait.is_some())
        })?;
        assert!(launcher.job.is_none());
        assert!(!launcher.completed);
        assert!(matches!(launcher.choice, Choice::Accepted));
        launcher.request_shutdown()?;
        finish(&mut launcher, &wake)
    }
}
