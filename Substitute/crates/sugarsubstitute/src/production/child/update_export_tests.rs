// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real receiving files: the report's text lands exactly, replaces a confirmed file, and every
//! output and worker retires before the writer reports itself settled.
use super::*;
use std::sync::Condvar;
use sugarsubstitute_desktop::report::surface::UpdateExport;
type TestResult = Result<(), Box<dyn Error>>;
/// Far no-progress guard against a hung export; never a pass/fail condition.
const HANG_GUARD: Duration = Duration::from_secs(600);

/// Counts execution wakes so a test waits for the next observed completion, never for time.
#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), sugarsubstitute_contracts::execution::ExecutionError> {
        if let Ok(mut count) = self.0.lock() {
            *count += 1;
        }
        self.1.notify_all();
        Ok(())
    }
}
impl Wake {
    fn count(&self) -> u64 {
        self.0.lock().map_or(0, |count| *count)
    }
    /// Block until a wake after `seen` arrives. The far guard only ends a hang: a test passes or
    /// fails on what it observes, never on elapsed time.
    fn after(&self, seen: u64) -> Result<(), Box<dyn Error>> {
        let mut count = self.0.lock().map_err(|_| "wake poisoned")?;
        while *count <= seen {
            let (next, waited) =
                self.1.wait_timeout(count, HANG_GUARD).map_err(|_| "wake poisoned")?;
            if waited.timed_out() && *next <= seen {
                return Err("no execution progress before the hang guard".into());
            }
            count = next;
        }
        Ok(())
    }
}

fn started(text: &str) -> Result<(UpdateReportExport, Arc<Wake>), Box<dyn Error>> {
    let wake = Arc::new(Wake::default());
    let mut export =
        UpdateReportExport::new(text.to_owned(), sugarsubstitute_execution::MonotonicClock::new())?;
    export.start(wake.clone())?;
    Ok((export, wake))
}

/// Drive one export to its result, waking only on observed completions.
fn finish(export: &mut UpdateReportExport, wake: &Wake) -> Result<(), Box<dyn Error>> {
    loop {
        let seen = wake.count();
        if let Some(result) = export.finished() {
            return result;
        }
        wake.after(seen)?;
    }
}

/// Close the writer and wait until its workers have retired.
fn settle(mut export: UpdateReportExport, wake: &Wake) -> TestResult {
    export.close();
    loop {
        let seen = wake.count();
        if export.settled() {
            assert!(export.owner.is_none(), "the writer's execution owner retired");
            return Ok(());
        }
        wake.after(seen)?;
    }
}

#[test]
fn the_report_text_lands_exactly_and_a_second_export_replaces_the_confirmed_file() -> TestResult {
    let directory = tempfile::tempdir()?;
    let destination = directory.path().join("SugarSubstitute update report.txt");
    let (mut export, wake) = started("Update to 2.4.1\nPhase: downloading\nCause: disk\n")?;
    export.begin(destination.clone())?;
    assert!(export.begin(destination.clone()).is_err(), "one write at a time");
    finish(&mut export, &wake)?;
    assert_eq!(std::fs::read(&destination)?, b"Update to 2.4.1\nPhase: downloading\nCause: disk\n");
    export.begin(destination.clone())?;
    finish(&mut export, &wake)?;
    assert_eq!(std::fs::read(&destination)?, b"Update to 2.4.1\nPhase: downloading\nCause: disk\n");
    let names: Vec<_> = std::fs::read_dir(directory.path())?.collect::<Result<_, _>>()?;
    assert_eq!(names.len(), 1, "no stage or backup file remains beside the report");
    assert!(export.settled(), "an open report with no write is settled");
    settle(export, &wake)
}

#[test]
fn a_destination_the_storage_owner_refuses_never_starts_a_write() -> TestResult {
    let (mut export, wake) = started("Update to 2.4.1\n")?;
    assert!(export.begin(PathBuf::from("relative.txt")).is_err());
    assert!(export.write.is_none());
    settle(export, &wake)
}

#[test]
fn a_closed_report_accepts_no_new_write_and_empty_text_has_no_writer() -> TestResult {
    assert!(
        UpdateReportExport::new(String::new(), sugarsubstitute_execution::MonotonicClock::new())
            .is_err()
    );
    let directory = tempfile::tempdir()?;
    let (mut export, wake) = started("Update to 2.4.1\n")?;
    export.close();
    assert!(export.begin(directory.path().join("report.txt")).is_err());
    assert!(std::fs::read_dir(directory.path())?.next().is_none());
    settle(export, &wake)
}
