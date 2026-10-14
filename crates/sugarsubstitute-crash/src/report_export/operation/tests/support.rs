// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual receiving files through central finite main/cleanup workers.
use super::super::*;
use crate::diagnostic_client::{DiagnosticRequestId, DiagnosticSelection};
use std::{
    error::Error,
    io,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use sugarsubstitute_contracts::{execution::*, incident::*, lifecycle::*};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock};
pub(super) type ResultT<T = ()> = Result<T, Box<dyn Error>>;
static NEXT: AtomicU64 = AtomicU64::new(1);
pub(super) fn destination() -> ResultT<PathBuf> {
    let root = std::env::temp_dir().join(format!(
        "sugar-report-export-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root)?;
    Ok(root.join("report.txt"))
}
#[derive(Default)]
struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl Wake {
    fn revision(&self) -> ResultT<u64> {
        Ok(*self.revision.lock().map_err(|_| "wake poisoned")?)
    }
    fn wait(&self, revision: u64) -> ResultT {
        let (value, timeout) = self
            .changed
            .wait_timeout_while(
                self.revision.lock().map_err(|_| "wake poisoned")?,
                Duration::from_secs(15),
                |value| *value == revision,
            )
            .map_err(|_| "wake wait poisoned")?;
        if timeout.timed_out() && *value == revision {
            return Err("original completion cutoff elapsed".into());
        }
        Ok(())
    }
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut revision = self.revision.lock().map_err(|_| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(io::Error::other("wake poisoned")),
            )
        })?;
        *revision += 1;
        self.changed.notify_all();
        Ok(())
    }
}
fn complete<T, E>(
    client: &ExecutionClient,
    wake: &Wake,
    job: &mut OwnedJob<T, E>,
) -> ResultT<JobCompletion<T, E>> {
    loop {
        let revision = wake.revision()?;
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(result) = job.try_take_completion() {
            return Ok(result);
        }
        wake.wait(revision)?;
    }
}
fn summary() -> ResultT<StoredIncidentSummary> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let seed = IncidentSeed::new(
        IncidentKey::new(IncidentSubject::Supervisor(owner)),
        IncidentRevision::new(1)?,
        BuildIdentity::new("export_native_fixture")?,
        IncidentUtc::from_unix_seconds(100)?,
        IncidentStage::Running,
    )?;
    Ok(StoredIncidentSummary::new(
        IncidentSummary::new(
            seed,
            IncidentRevision::new(2)?,
            SummaryFacts::new(
                RetentionFacts::new(IncidentUtc::from_unix_seconds(101)?, None)?,
                ExitAttribution::Supervisor { status: 37 },
                FailureEvidence::Unexplained,
                CaptureEvidence::Unavailable(CaptureAbsence::Missing),
                PayloadDisposition::Unavailable,
                ReportDisposition::new(0, None)?,
                SymbolOutcome::NotRequested,
            ),
        )?,
        None,
    )?)
}
fn spec(id: u64, class: JobClass, bytes: u64, deadline: Option<Moment>) -> ResultT<JobSpec> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::Infrastructure,
        class,
        deadline,
        reservation_bytes: bytes,
        replacement: None,
    })
}
struct TrackedView {
    view: ReportView,
    dropped: Arc<AtomicBool>,
}
impl std::borrow::Borrow<ReportView> for TrackedView {
    fn borrow(&self) -> &ReportView {
        &self.view
    }
}
impl Drop for TrackedView {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}
pub(super) struct Observed {
    pub(super) outcome: Result<(), C>,
    pub(super) receipt: Option<ReportExportReceipt>,
    pub(super) canonical: Vec<u8>,
    pub(super) retained: ReportExportCustody,
}
pub(super) fn export(
    path: &Path,
    overwrite: bool,
    stale: bool,
    include_dump: bool,
    cancel: bool,
) -> ResultT<Observed> {
    use std::os::windows::ffi::OsStrExt;
    let clock = MonotonicClock::new();
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let preparation =
        client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let main_scope =
        Arc::new(client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?);
    let cleanup_scope =
        client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    let current = summary()?;
    let record = current.clone();
    let binding = ReportBinding::new(
        current.summary().seed().key(),
        current.summary().revision(),
        ReportSessionId::new(1)?,
    );
    let mut prepare = client.submit_blocking(
        &preparation,
        spec(1, JobClass::InfrastructureOrdinary, 2_097_152, None)?,
        move |context| {
            ReportView::format(
                record.summary(),
                binding,
                SummaryEvictions::new(
                    binding.incident().subject().coordinator().namespace,
                    0,
                    0,
                    0,
                    None,
                )
                .map_err(io::Error::other)?,
                &[],
                "en",
                context.memory(),
            )
            .map_err(io::Error::other)
        },
    )?;
    let completion = complete(&client, &wake, &mut prepare)?;
    let JobOutcome::Success(view) = completion.outcome else {
        return Err("canonical view factory failed".into());
    };
    drop(completion.memory);
    let canonical = view.text().as_bytes().to_vec();
    let destination: Vec<u8> = path.as_os_str().encode_wide().flat_map(u16::to_le_bytes).collect();
    let selected_binding = if stale {
        ReportBinding::new(binding.incident(), IncidentRevision::new(3)?, binding.session())
    } else {
        binding
    };
    let intent = ReportExportIntent::new(
        selected_binding,
        DiagnosticSelection {
            id: DiagnosticRequestId::new(1)?,
            selection: if include_dump {
                ExportSelection::TextAndNativeDump
            } else {
                ExportSelection::TextOnly
            },
            overwrite,
            destination: &destination,
        },
    )?;
    let dropped = Arc::new(AtomicBool::new(false));
    let tracked = TrackedView { view, dropped: dropped.clone() };
    let shared: Arc<Mutex<Option<ReportExportOperation<TrackedView>>>> = Arc::new(Mutex::new(None));
    let cleanup_shared = shared.clone();
    let cleanup_client = client.clone();
    let cutoff = clock.now().checked_add(Duration::from_secs(30))?;
    let main_spec = spec(3, JobClass::InfrastructureOrdinary, 131_072, Some(cutoff))?;
    let permit = client
        .reserve_report_export_cleanup(
            &cleanup_scope,
            spec(2, JobClass::InfrastructureLifecycle, 32_768, None)?,
            &main_scope,
            main_spec,
            move |context| {
                let mut shared =
                    cleanup_shared.lock().map_err(|_| io::Error::other("export owner poisoned"))?;
                if let Some(operation) = shared.as_mut() {
                    let proof = cleanup_client
                        .report_export_retirement(
                            operation.original_origin().map_err(io::Error::other)?,
                            &context,
                        )
                        .map_err(io::Error::other)?;
                    operation.retire_receiving(&proof).map_err(|_| {
                        io::Error::other("original export cleanup failure retained")
                    })?;
                }
                Ok::<_, io::Error>(())
            },
        )
        .map_err(|failure| failure.error)?;
    let main_shared = shared.clone();
    let main_client = client.clone();
    let cancel_scope = main_scope.clone();
    let mut main = client
        .submit_report_export(&main_scope, main_spec, &permit, move |context| {
            let admission =
                main_client.report_export_admission(&context).map_err(io::Error::other)?;
            let operation = ReportExportOperation::new(admission, intent, tracked, None)
                .map_err(|failure| io::Error::other(failure.0))?;
            let mut shared =
                main_shared.lock().map_err(|_| io::Error::other("export owner poisoned"))?;
            *shared = Some(operation);
            if cancel {
                cancel_scope.cancel();
            }
            let result = shared
                .as_mut()
                .ok_or_else(|| io::Error::other("actual export absent"))?
                .execute(&current);
            Ok::<_, io::Error>(result)
        })
        .map_err(|failure| failure.error)?;
    let completion = complete(&client, &wake, &mut main)?;
    let outcome = match completion.outcome {
        JobOutcome::Success(result) => result,
        JobOutcome::Cancelled if cancel => Err(C::Control),
        other => return Err(format!("unexpected central export outcome: {other:?}").into()),
    };
    drop(completion.memory);
    assert!(!dropped.load(Ordering::SeqCst));
    let retained = {
        let mut shared = shared.lock().map_err(|_| "export owner poisoned")?;
        let operation = shared.as_mut().ok_or("actual export missing")?;
        assert!(operation.receipt().is_none());
        assert!(operation.take_view().is_none());
        operation.custody()
    };
    let cleanup_deadline =
        client.freeze_report_export_cleanup_deadline(&permit, Duration::from_secs(15))?;
    let mut cleanup = permit.begin(cleanup_deadline).map_err(|failure| failure.error)?;
    let completion = complete(&client, &wake, &mut cleanup)?;
    if !matches!(completion.outcome, JobOutcome::Success(())) {
        return Err("actual export cleanup failed".into());
    }
    drop(completion.memory);
    let mut operation = shared
        .lock()
        .map_err(|_| "export owner poisoned")?
        .take()
        .ok_or("actual export missing")?;
    let receipt = operation.receipt();
    assert_eq!(operation.custody().native_outputs, 0);
    if outcome.is_err() {
        assert!(operation.original_failure().is_some());
    }
    // Transfer only the complete presentation owner after physical cleanup.
    assert!(!dropped.load(Ordering::SeqCst));
    let returned = operation.take_view().ok_or("whole checked-retired view missing")?;
    assert_eq!(returned.view.text().as_bytes(), canonical);
    drop(operation);
    assert!(!dropped.load(Ordering::SeqCst));
    drop(returned);
    assert!(dropped.load(Ordering::SeqCst));
    drop(owner);
    Ok(Observed { outcome, receipt, canonical, retained })
}

mod native_probe;
pub(super) use native_probe::predecessor_mutation;
