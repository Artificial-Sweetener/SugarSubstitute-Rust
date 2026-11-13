// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Write the update report the window shows to a file the user selected, on the startup
//! execution owner.
//!
//! Each export is one admitted report-export pair: the main job writes the report through
//! [`ReportFile`] and activates it in place of any earlier file, and its cleanup job, held before
//! the main job is admitted, then closes every handle the write opened, whatever the write's
//! outcome. Each write takes the report text the window shows and copies, so the file holds
//! exactly what the user saw.
use crate::job_sequence::JobSequence;
use std::{
    cell::Cell,
    error::Error,
    io,
    os::windows::ffi::OsStrExt,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::{
    CompletionWake, ExecutionPort, JobClass, JobContext, JobOutcome, JobSpec, Operation, OwnedJob,
    Scope, ScopeSpec,
};
use sugarsubstitute_desktop::report::surface::UpdateExport;
use sugarsubstitute_execution::{
    ExecutionClient, REPORT_EXPORT_BYTES, REPORT_EXPORT_CLEANUP_BYTES, ReportExportCleanupPermit,
};
use sugarsubstitute_supervision::{ReportFile, ReportFileSelection};

/// Longest one write of a report may run before the execution owner stops it.
const WRITE_BUDGET: Duration = Duration::from_secs(30);
/// Longest the paired cleanup may run; the execution owner allows at most fifteen seconds.
const CLEANUP_BUDGET: Duration = Duration::from_secs(15);
/// Largest chunk the receiving file accepts in one write.
const CHUNK_BYTES: usize = 64 * 1024;

/// The receiving file a write opened, shared with the cleanup that closes it.
type Destination = Arc<Mutex<Option<ReportFile>>>;

/// Scopes for every update-report export of this run, created on the first export.
struct ExportScopes {
    main: Scope,
    cleanup: Scope,
}

/// The startup host's owner of update-report exports. Writers share its scopes and report to it
/// while their jobs are outstanding, so execution shutdown waits for them.
pub(super) struct UpdateReports {
    main: ScopeSpec,
    cleanup: ScopeSpec,
    scopes: Option<Rc<ExportScopes>>,
    outstanding: Rc<Cell<usize>>,
}

impl UpdateReports {
    /// Exports on the `main` and `cleanup` scopes, which process composition assigns.
    #[cfg(any(feature = "windows-native-host", test))]
    pub(super) fn new(main: ScopeSpec, cleanup: ScopeSpec) -> Self {
        Self { main, cleanup, scopes: None, outstanding: Rc::default() }
    }

    /// A writer for one opened report.
    /// # Errors
    /// Preserves scope creation failures.
    pub(super) fn writer(
        &mut self,
        client: ExecutionClient,
        jobs: JobSequence,
    ) -> Result<UpdateReportFile, Box<dyn Error>> {
        let scopes = match &self.scopes {
            Some(scopes) => scopes.clone(),
            None => {
                let scopes = Rc::new(ExportScopes {
                    main: client.create_scope(self.main, None)?,
                    cleanup: client.create_scope(self.cleanup, None)?,
                });
                self.scopes = Some(scopes.clone());
                scopes
            }
        };
        Ok(UpdateReportFile {
            client,
            jobs,
            scopes,
            stage: Stage::Idle,
            closed: false,
            outstanding: self.outstanding.clone(),
        })
    }

    /// Whether no export has a job outstanding.
    pub(super) fn settled(&self) -> bool {
        self.outstanding.get() == 0
    }
}

/// Where one writer's export stands.
enum Stage {
    Idle,
    /// The main job writes; its cleanup is held until it ends.
    Writing {
        permit: ReportExportCleanupPermit<(), io::Error>,
        main: OwnedJob<(), io::Error>,
        destination: Destination,
    },
    /// The write ended with `outcome`; the cleanup closes what it opened.
    Closing {
        cleanup: OwnedJob<(), io::Error>,
        destination: Destination,
        outcome: Result<(), Box<dyn Error>>,
    },
    /// The main job was refused, and the report already showed it; the held cleanup ends unrun.
    Withdrawing {
        cleanup: OwnedJob<(), io::Error>,
    },
    /// The export ended; the report takes the outcome once.
    Finished(Result<(), Box<dyn Error>>),
}

/// Writes one opened update report to the files the user selects, one write at a time.
pub(super) struct UpdateReportFile {
    client: ExecutionClient,
    jobs: JobSequence,
    scopes: Rc<ExportScopes>,
    stage: Stage,
    closed: bool,
    outstanding: Rc<Cell<usize>>,
}

impl UpdateReportFile {
    /// Admit the cleanup and then the write of `report` to `destination`.
    fn admit(&mut self, destination: PathBuf, report: &str) -> Result<(), Box<dyn Error>> {
        let units: Vec<u8> =
            destination.as_os_str().encode_wide().flat_map(u16::to_le_bytes).collect();
        // The cleanup's identity precedes its main job's, as the execution owner requires.
        let cleanup_id = self.jobs.allocate()?;
        let main_id = self.jobs.allocate()?;
        // The shell's save dialog already asked before the selection replaces a file.
        let selection = ReportFileSelection::new(&units, true, [cleanup_id.get(), main_id.get()])?;
        let main_spec = JobSpec {
            id: main_id,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(self.client.now()?.checked_add(WRITE_BUDGET)?),
            reservation_bytes: REPORT_EXPORT_BYTES,
            replacement: None,
        };
        let cleanup_spec = JobSpec {
            id: cleanup_id,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureLifecycle,
            deadline: None,
            reservation_bytes: REPORT_EXPORT_CLEANUP_BYTES,
            replacement: None,
        };
        let destination = Destination::default();
        let (client, closing) = (self.client.clone(), destination.clone());
        let permit = self
            .client
            .reserve_report_export_cleanup(
                &self.scopes.cleanup,
                cleanup_spec,
                &self.scopes.main,
                main_spec,
                move |context| close(&client, &closing, &context),
            )
            .map_err(|rejection| rejection.error)?;
        self.outstanding.set(self.outstanding.get() + 1);
        let (client, writing) = (self.client.clone(), destination.clone());
        let report: Arc<[u8]> = Arc::from(report.as_bytes());
        match self.client.submit_report_export(
            &self.scopes.main,
            main_spec,
            &permit,
            move |context| write(&client, &writing, &selection, &report, &context),
        ) {
            Ok(main) => {
                self.stage = Stage::Writing { permit, main, destination };
                Ok(())
            }
            Err(rejection) => {
                self.stage = Stage::Withdrawing { cleanup: permit.cancel_and_observe() };
                Err(Box::new(rejection.error))
            }
        }
    }

    /// Move the export on by every terminal outcome its jobs have delivered.
    fn advance(&mut self) {
        loop {
            let stage = std::mem::replace(&mut self.stage, Stage::Idle);
            let (next, moved) = match stage {
                Stage::Writing { permit, mut main, destination } => {
                    match main.try_take_completion() {
                        Some(completion) => {
                            let outcome = outcome(completion.outcome, "write");
                            (begin_cleanup(&self.client, permit, destination, outcome), true)
                        }
                        None => (Stage::Writing { permit, main, destination }, false),
                    }
                }
                Stage::Closing { mut cleanup, destination, outcome } => {
                    match cleanup.try_take_completion() {
                        Some(completion) => {
                            self.retire_one();
                            (
                                Stage::Finished(closed(completion.outcome, &destination, outcome)),
                                true,
                            )
                        }
                        None => (Stage::Closing { cleanup, destination, outcome }, false),
                    }
                }
                Stage::Withdrawing { mut cleanup } => match cleanup.try_take_completion() {
                    Some(_) => {
                        self.retire_one();
                        (Stage::Idle, true)
                    }
                    None => (Stage::Withdrawing { cleanup }, false),
                },
                stage @ (Stage::Idle | Stage::Finished(_)) => (stage, false),
            };
            self.stage = next;
            if !moved {
                return;
            }
        }
    }

    fn retire_one(&self) {
        self.outstanding.set(self.outstanding.get().saturating_sub(1));
    }
}

impl UpdateExport for UpdateReportFile {
    /// The startup host's own execution wake turns the window's loop when a job ends, and the
    /// report polls this writer on every turn, so no further wake is needed.
    fn start(&mut self, _wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn begin(&mut self, destination: PathBuf, report: &str) -> Result<(), Box<dyn Error>> {
        self.advance();
        if self.closed {
            return Err("The update report is closing".into());
        }
        if !matches!(self.stage, Stage::Idle) {
            return Err("An update report export is still being written".into());
        }
        self.admit(destination, report)
    }

    fn finished(&mut self) -> Option<Result<(), Box<dyn Error>>> {
        self.advance();
        match std::mem::replace(&mut self.stage, Stage::Idle) {
            Stage::Finished(outcome) => Some(outcome),
            stage => {
                self.stage = stage;
                None
            }
        }
    }

    fn close(&mut self) {
        self.closed = true;
    }

    fn settled(&mut self) -> bool {
        self.advance();
        matches!(self.stage, Stage::Idle | Stage::Finished(_))
    }
}

/// Activate the held cleanup with its first deadline. When it cannot begin, the write's handles
/// stay open until the process exits; the export reports that as a failure.
fn begin_cleanup(
    client: &ExecutionClient,
    permit: ReportExportCleanupPermit<(), io::Error>,
    destination: Destination,
    outcome: Result<(), Box<dyn Error>>,
) -> Stage {
    let deadline = match client.freeze_report_export_cleanup_deadline(&permit, CLEANUP_BUDGET) {
        Ok(deadline) => deadline,
        Err(cause) => return withdrawn(permit.cancel_and_observe(), destination, &cause),
    };
    match permit.begin(deadline) {
        Ok(cleanup) => Stage::Closing { cleanup, destination, outcome },
        Err(rejection) => {
            withdrawn(rejection.permit.cancel_and_observe(), destination, &rejection.error)
        }
    }
}

fn withdrawn(
    cleanup: OwnedJob<(), io::Error>,
    destination: Destination,
    cause: &dyn std::fmt::Display,
) -> Stage {
    tracing::error!(operation = "update_report_export_cleanup", cause = %cause,
        "Update report export cleanup could not begin");
    Stage::Closing {
        cleanup,
        destination,
        outcome: Err(format!("The export's cleanup could not begin: {cause}").into()),
    }
}

/// The write's outcome, unchanged when every handle it opened is closed.
fn closed(
    cleanup: JobOutcome<(), io::Error>,
    destination: &Destination,
    written: Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let released = destination
        .lock()
        .map(|file| file.as_ref().is_none_or(ReportFile::closed))
        .unwrap_or(false);
    match (outcome(cleanup, "cleanup"), released) {
        (Ok(()), true) => written,
        (Ok(()), false) => Err("The exported file's handles remain open".into()),
        (Err(cause), _) => {
            if let Err(write) = &written {
                tracing::error!(operation = "update_report_export", cause = %write,
                    "Update report export failed before its cleanup failed");
            }
            Err(cause)
        }
    }
}

fn outcome(outcome: JobOutcome<(), io::Error>, step: &str) -> Result<(), Box<dyn Error>> {
    match outcome {
        JobOutcome::Success(()) => Ok(()),
        JobOutcome::Failure(cause) => Err(Box::new(cause)),
        JobOutcome::Cancelled => Err(format!("The export's {step} was cancelled").into()),
        JobOutcome::TimedOut => Err(format!("The export's {step} ran out of time").into()),
        JobOutcome::Panicked(cause) => {
            Err(format!("The export's {step} panicked: {cause:?}").into())
        }
        other => Err(format!("The export's {step} ended unexpectedly: {other:?}").into()),
    }
}

/// Main job: write the report to the selected file and activate it.
fn write(
    client: &ExecutionClient,
    destination: &Destination,
    selection: &ReportFileSelection,
    report: &[u8],
    context: &JobContext,
) -> io::Result<()> {
    let admission = client.report_export_admission(context).map_err(io::Error::other)?;
    let mut held =
        destination.lock().map_err(|_| io::Error::other("Export file owner poisoned"))?;
    let file = held.insert(ReportFile::new(admission.into_origin()));
    file.open(selection).map_err(io::Error::other)?;
    for chunk in report.chunks(CHUNK_BYTES) {
        file.write(chunk).map_err(io::Error::other)?;
    }
    file.flush().map_err(io::Error::other)?;
    file.activate().map_err(io::Error::other)
}

/// Cleanup job: retire and close everything the write opened.
fn close(
    client: &ExecutionClient,
    destination: &Destination,
    context: &JobContext,
) -> io::Result<()> {
    let mut held =
        destination.lock().map_err(|_| io::Error::other("Export file owner poisoned"))?;
    let Some(file) = held.as_mut() else { return Ok(()) };
    let proof = client
        .report_export_retirement(file.origin().map_err(io::Error::other)?, context)
        .map_err(io::Error::other)?;
    file.retire(&proof).map_err(io::Error::other)
}

#[cfg(test)]
#[path = "update_report/tests.rs"]
mod tests;
