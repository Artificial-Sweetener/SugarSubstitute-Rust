// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Record a close's unsaved work once, report it exactly once, and keep unreadable records.
use super::*;
use std::time::Duration;
use sugarsubstitute_application::workspace_session::close_save::{
    CloseReason, CloseSaveOutcome, NotSavedLastTime, SaveFailure, SaveVerdict, UnsavedCause,
    UnsavedItem,
};

impl Fixture {
    fn record(
        &self,
        outcome: CloseSaveOutcome,
        ended: Moment,
    ) -> TestResult<Result<(), StoreError>> {
        let directory = self.directory.path().to_path_buf();
        let job = self.client.submit_blocking(&self.scope, self.spec()?, move |context| {
            close_record::write_worker(&directory, &outcome, ended, &context)
        })?;
        match self.terminal(job)? {
            JobOutcome::Success(()) => Ok(Ok(())),
            JobOutcome::Failure(cause) => Ok(Err(cause)),
            other => Err(format!("Record stopped: {:?}", other.kind()).into()),
        }
    }
    fn notice(&self) -> TestResult<Result<Option<NotSavedLastTime>, StoreError>> {
        let directory = self.directory.path().to_path_buf();
        let job = self.client.submit_blocking(&self.scope, self.spec()?, move |context| {
            close_record::take_notice_worker(&directory, &context)
        })?;
        match self.terminal(job)? {
            JobOutcome::Success(notice) => Ok(Ok(notice)),
            JobOutcome::Failure(cause) => Ok(Err(cause)),
            other => Err(format!("Notice stopped: {:?}", other.kind()).into()),
        }
    }
}

#[test]
fn an_unsaved_close_is_reported_once_at_the_next_start_and_its_record_is_kept() -> TestResult {
    let fixture = Fixture::new()?;
    assert!(fixture.notice()??.is_none(), "no record means no notice");
    let outcome = CloseSaveOutcome {
        reason: CloseReason::RestartToApply,
        workspace: SaveVerdict::Stale { since: Moment::from_nanos(1_000_000_000) },
        inputs: SaveVerdict::Failed(SaveFailure::Storage),
    };
    fixture.record(outcome, Moment::from_nanos(3_500_000_000))??;
    let notice = fixture.notice()??.ok_or("The unsaved close produced no notice")?;
    assert_eq!(
        notice,
        NotSavedLastTime {
            reason: CloseReason::RestartToApply,
            items: vec![
                UnsavedItem::Workspace(UnsavedCause::Interrupted {
                    unsaved_for: Duration::from_millis(2_500)
                }),
                UnsavedItem::InputCanvas(UnsavedCause::Failed(SaveFailure::Storage)),
            ],
        }
    );
    assert!(fixture.notice()??.is_none(), "the notice appears once");
    assert!(fixture.directory.path().join(close_record::CLOSE_RECORD).is_file());
    Ok(())
}

#[test]
fn an_unreadable_close_record_is_refused_and_left_untouched() -> TestResult {
    let fixture = Fixture::new()?;
    let path = fixture.directory.path().join(close_record::CLOSE_RECORD);
    let future = br#"{"format":"sugarsubstitute.close-save","version":2,"notified":false}"#;
    std::fs::write(&path, future)?;
    assert!(fixture.notice()?.is_err());
    assert_eq!(std::fs::read(&path)?, future);
    Ok(())
}
