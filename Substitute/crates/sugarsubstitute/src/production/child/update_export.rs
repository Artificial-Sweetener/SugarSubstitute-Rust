// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Export the open update report to the file the user chose in the Save dialog.
//!
//! One writer serves one opened report. It runs its own short-lived execution owner, started
//! with the report window's wake, so every job completion wakes exactly the surface that waits
//! for it. Each export is the execution owner's report-export pair: an independent close-only
//! cleanup job is held first, then the main job takes the report-export admission and writes
//! through the storage owner's receiving file (stage, flush, no-replace activation). When the
//! main job ends the cleanup job runs, rolls back anything not activated, and closes every
//! native output. Only then is the write finished. The text is the report's plain text, the
//! same the Copy action puts on the clipboard.
use std::{
    error::Error,
    io,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, JobClass, JobId, JobOutcome, JobSpec, Moment, Operation, OwnedJob,
    Scope, ScopeId, ScopeSpec,
};
use sugarsubstitute_execution::{
    ExecutionClient, ExecutionOwner, REPORT_EXPORT_BYTES, REPORT_EXPORT_CLEANUP_BYTES,
    ReportExportCleanupPermit,
};
use sugarsubstitute_supervision::{ReportFile, ReportFileSelection};

/// Bound of one export's main job: open, write, flush and activate a small text file.
const WRITE: Duration = Duration::from_secs(30);
/// Bound of one export's cleanup job, the most the execution owner admits.
const CLEANUP: Duration = Duration::from_secs(15);
/// Bound of the writer's own execution shutdown after its report closed.
const SHUTDOWN: Duration = Duration::from_secs(15);
/// Largest chunk the receiving file accepts in one write.
const CHUNK: usize = 64 * 1024;
/// Largest report text exported; reports are bounded far below this.
const MAX_TEXT: usize = 1024 * 1024;

/// The receiving file both jobs of one export reach: the main job creates it, cleanup retires it.
type Receiving = Arc<Mutex<Option<ReportFile>>>;

/// One export in flight.
enum Write {
    /// The main job runs; its cleanup is held until it ends.
    Main {
        job: OwnedJob<(), io::Error>,
        cleanup: ReportExportCleanupPermit<(), io::Error>,
        receiving: Receiving,
    },
    /// The main job ended with `main`; cleanup now retires every output.
    Cleanup { main: Result<(), String>, job: OwnedJob<(), io::Error>, receiving: Receiving },
}

/// Writes one opened update report to user-selected files.
pub(in crate::production) struct UpdateReportExport {
    text: Vec<u8>,
    clock: Arc<dyn Clock>,
    owner: Option<ExecutionOwner>,
    scopes: Option<(Scope, Scope)>,
    jobs: u64,
    write: Option<Write>,
    finished: Option<Result<(), String>>,
    closed: bool,
    shutdown_requested: bool,
}

impl UpdateReportExport {
    /// A writer for `text`, the report's plain text, measured on `clock`.
    /// # Errors
    /// Rejects empty or oversized text.
    pub(in crate::production) fn new(text: String, clock: Arc<dyn Clock>) -> io::Result<Self> {
        if text.is_empty() || text.len() > MAX_TEXT {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Update report text bounds"));
        }
        Ok(Self {
            text: text.into_bytes(),
            clock,
            owner: None,
            scopes: None,
            jobs: 0,
            write: None,
            finished: None,
            closed: false,
            shutdown_requested: false,
        })
    }

    fn client(&self) -> Result<ExecutionClient, Box<dyn Error>> {
        Ok(self.owner.as_ref().ok_or("Update report export has not started")?.client())
    }

    fn next_job(&mut self) -> Result<JobId, Box<dyn Error>> {
        self.jobs = self.jobs.checked_add(1).ok_or("Update report export jobs exhausted")?;
        Ok(JobId::new(self.jobs)?)
    }

    /// Admit the held cleanup, then the main job, for one export to `destination`.
    fn submit(&mut self, destination: PathBuf) -> Result<Write, Box<dyn Error>> {
        use std::os::windows::ffi::OsStrExt;
        let client = self.client()?;
        let bytes: Vec<u8> =
            destination.as_os_str().encode_wide().flat_map(u16::to_le_bytes).collect();
        let identity = [u64::from(std::process::id()), self.jobs.saturating_add(1)];
        // The shell Save dialog asked before replacing an existing file.
        let selection = ReportFileSelection::new(&bytes, true, identity)?;
        let cleanup_spec = JobSpec {
            id: self.next_job()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureLifecycle,
            deadline: None,
            reservation_bytes: REPORT_EXPORT_CLEANUP_BYTES,
            replacement: None,
        };
        let main_spec = JobSpec {
            id: self.next_job()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(client.now()?.checked_add(WRITE)?),
            reservation_bytes: REPORT_EXPORT_BYTES,
            replacement: None,
        };
        let (main_scope, cleanup_scope) =
            self.scopes.as_ref().ok_or("Update report export scopes are absent")?;
        let receiving: Receiving = Arc::new(Mutex::new(None));
        let cleanup = client
            .reserve_report_export_cleanup(
                cleanup_scope,
                cleanup_spec,
                main_scope,
                main_spec,
                retire(client.clone(), receiving.clone()),
            )
            .map_err(|rejected| rejected.error)?;
        let text = self.text.clone();
        let writer = client.clone();
        let target = receiving.clone();
        let submitted =
            client.submit_report_export(main_scope, main_spec, &cleanup, move |context| {
                let admission =
                    writer.report_export_admission(&context).map_err(io::Error::other)?;
                let mut held = target.lock().map_err(|_| io::Error::other("Export poisoned"))?;
                let file = held.insert(ReportFile::new(admission.into_origin()));
                file.open(&selection).map_err(io::Error::other)?;
                for chunk in text.chunks(CHUNK) {
                    file.write(chunk).map_err(io::Error::other)?;
                }
                file.flush().map_err(io::Error::other)?;
                file.activate().map_err(io::Error::other)
            });
        match submitted {
            Ok(job) => Ok(Write::Main { job, cleanup, receiving }),
            Err(rejected) => {
                // Nothing ran; the held cleanup is cancelled and observed to its end.
                let job = cleanup.cancel_and_observe();
                Ok(Write::Cleanup { main: Err(rejected.error.to_string()), job, receiving })
            }
        }
    }

    /// Move the export forward as far as its jobs allow; a finished export leaves its result.
    fn advance(&mut self) {
        let Some(write) = self.write.take() else { return };
        self.write = match write {
            Write::Main { mut job, cleanup, receiving } => match job.try_take_completion() {
                None => Some(Write::Main { job, cleanup, receiving }),
                Some(completion) => {
                    let main = outcome(completion.outcome);
                    Some(self.begin_cleanup(main, cleanup, receiving))
                }
            },
            Write::Cleanup { main, mut job, receiving } => match job.try_take_completion() {
                None => Some(Write::Cleanup { main, job, receiving }),
                Some(completion) => {
                    let retired = outcome(completion.outcome).and_then(|()| {
                        let held = receiving.lock().map_err(|_| "Export poisoned".to_owned())?;
                        if held.as_ref().is_none_or(ReportFile::closed) {
                            Ok(())
                        } else {
                            Err("The exported file's outputs are still open".to_owned())
                        }
                    });
                    self.finished = Some(main.and(retired));
                    None
                }
            },
        };
    }

    /// Run the held cleanup with its first fixed deadline; one that cannot run is cancelled.
    fn begin_cleanup(
        &self,
        main: Result<(), String>,
        cleanup: ReportExportCleanupPermit<(), io::Error>,
        receiving: Receiving,
    ) -> Write {
        let deadline = self.client().map_err(|cause| cause.to_string()).and_then(|client| {
            client
                .freeze_report_export_cleanup_deadline(&cleanup, CLEANUP)
                .map_err(|cause| cause.to_string())
        });
        let job = match deadline {
            Ok(deadline) => match cleanup.begin(deadline) {
                Ok(job) => job,
                Err(rejected) => rejected.permit.cancel_and_observe(),
            },
            Err(cause) => {
                tracing::error!(operation = "update_report_export_cleanup", cause = %cause,
                    "The export's cleanup could not be scheduled");
                cleanup.cancel_and_observe()
            }
        };
        Write::Cleanup { main, job, receiving }
    }
}

/// The cleanup job: retire the receiving file the main job created, if it created one.
fn retire(
    client: ExecutionClient,
    receiving: Receiving,
) -> impl FnOnce(sugarsubstitute_contracts::execution::JobContext) -> Result<(), io::Error>
+ Send
+ 'static {
    move |context| {
        let mut held = receiving.lock().map_err(|_| io::Error::other("Export poisoned"))?;
        let Some(file) = held.as_mut() else { return Ok(()) };
        let origin = file.origin().map_err(io::Error::other)?;
        let proof = client.report_export_retirement(origin, &context).map_err(io::Error::other)?;
        file.retire(&proof).map_err(io::Error::other)
    }
}

/// A job's outcome as the report shows it: success, or the cause in words.
fn outcome(outcome: JobOutcome<(), io::Error>) -> Result<(), String> {
    match outcome {
        JobOutcome::Success(()) => Ok(()),
        JobOutcome::Failure(cause) => Err(cause.to_string()),
        other => Err(format!("{:?}", other.kind())),
    }
}

impl sugarsubstitute_desktop::report::surface::UpdateExport for UpdateReportExport {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        if self.owner.is_some() || self.closed {
            return Err("Update report export cannot restart".into());
        }
        let owner = ExecutionOwner::new(self.clock.clone(), wake)?;
        let client = owner.client();
        self.owner = Some(owner);
        use sugarsubstitute_contracts::execution::ExecutionPort;
        let main = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let cleanup =
            client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
        self.scopes = Some((main, cleanup));
        Ok(())
    }

    fn begin(&mut self, destination: PathBuf) -> Result<(), Box<dyn Error>> {
        if self.closed || self.write.is_some() || self.shutdown_requested {
            return Err("An update report export is already running or the report closed".into());
        }
        self.finished = None;
        self.write = Some(self.submit(destination)?);
        self.advance();
        Ok(())
    }

    fn finished(&mut self) -> Option<Result<(), Box<dyn Error>>> {
        self.advance();
        self.finished.take().map(|result| result.map_err(Into::into))
    }

    fn close(&mut self) {
        self.closed = true;
    }

    fn settled(&mut self) -> bool {
        self.advance();
        if self.write.is_some() {
            return false;
        }
        if !self.closed {
            return true;
        }
        let Some(owner) = &mut self.owner else { return true };
        if !self.shutdown_requested {
            self.shutdown_requested = true;
            self.scopes = None;
            let deadline = self.clock.now().checked_add(SHUTDOWN).ok();
            owner.request_shutdown(deadline.unwrap_or(Moment::from_nanos(u64::MAX)));
        }
        match owner.poll_shutdown() {
            Ok(progress) if progress.settled => {
                if let Err(cause) = owner.finish_shutdown() {
                    tracing::error!(operation = "update_report_export_shutdown", cause = %cause,
                        "The update report export's workers did not retire");
                    return false;
                }
                self.owner = None;
                true
            }
            Ok(_) => false,
            Err(cause) => {
                tracing::error!(operation = "update_report_export_shutdown", cause = %cause,
                    "The update report export's shutdown failed");
                false
            }
        }
    }
}

#[cfg(test)]
#[path = "update_export_tests.rs"]
mod tests;
