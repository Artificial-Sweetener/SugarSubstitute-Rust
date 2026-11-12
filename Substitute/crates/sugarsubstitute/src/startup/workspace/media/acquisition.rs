// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify originals within an owned load/acquisition lifetime, independently of HTTP deferrals.
use super::*;
/// One finite verification whose terminal remains observed even after its HTTP caller expires.
pub(super) struct Acquiring {
    asset: SessionAsset,
    /// Actual execution-owned verification/checked-handle custody.
    pub(super) job: OwnedJob<(), StoreError>,
}
/// The load worker retains checked handles directly in the eventual media owner's registry.
pub(in crate::startup::workspace) struct Preloader {
    readers: Arc<Mutex<BTreeMap<String, Verified>>>,
}
impl Preloader {
    /// Verify all large current originals before the page can issue its first tiny range.
    /// The shared8GiB asset bound permits fewer than32 originals above256MiB; these handles
    /// remain pinned while smaller reader entries can be evicted with checked retirement.
    pub(in crate::startup::workspace) fn prepare(
        &self,
        store: &WorkspaceStore,
        restored: &workspace_session::RestoredWorkspace,
        resident: &MemoryLease,
        context: &JobContext,
    ) -> Result<(), StoreError> {
        let Some((_, checkpoint)) = &restored.checkpoint else {
            return Ok(());
        };
        for asset in checkpoint.assets.iter().filter(|asset| asset.bytes > LARGE_ORIGINAL_BYTES) {
            reader::controls(context)?;
            if let Err(cause) =
                reader::prepare(&self.readers, &store.asset_path(asset)?, asset, resident, context)
            {
                reader::controls(context)?;
                tracing::warn!(operation="workspace_original_preload",digest=%asset.digest,cause=%cause,"Retained original unavailable; saved reference preserved");
            }
        }
        Ok(())
    }
}
impl MediaWork {
    /// Loan the existing registry to the pre-main admitted load worker.
    pub(in crate::startup::workspace) fn preloader(&self) -> Preloader {
        Preloader { readers: self.readers.clone() }
    }
    /// Start one independent finite verification. It ends on its observed outcome; only the
    /// shutdown cutoff bounds it, while each deferred HTTP request keeps the deadline its page set.
    pub(super) fn acquire(
        &mut self,
        admission: MediaAdmission<'_>,
        path: PathBuf,
        asset: SessionAsset,
        resident: MemoryLease,
    ) -> Result<(), Box<dyn Error>> {
        let MediaAdmission { client, scope, jobs, cutoff, .. } = admission;
        if self.acquisition.is_some() || !self.running.is_empty() {
            return Ok(());
        }
        let readers = self.readers.clone();
        let expected = asset.clone();
        let spec = JobSpec {
            id: jobs.allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(cutoff.unwrap_or(crate::job_deadline::OPEN_ENDED)),
            reservation_bytes: 128 * 1024,
            replacement: None,
        };
        match client.submit_blocking(scope, spec, move |context| {
            reader::prepare(&readers, &path, &expected, &resident, &context)
        }) {
            Ok(job) => self.acquisition = Some(Acquiring { asset, job }),
            Err(
                AdmissionError::MemorySaturated { .. }
                | AdmissionError::QueueSaturated { .. }
                | AdmissionError::CompletionSaturated,
            ) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
    /// Observe the actual terminal before admitting any HTTP read or retiring its handles.
    pub(super) fn observe_acquisition(
        &mut self,
        driver: Option<&WorkspaceDriver>,
        commands: &mut VecDeque<DesktopStartupCommand>,
    ) -> Result<(), Box<dyn Error>> {
        let Some(active) = &mut self.acquisition else {
            return Ok(());
        };
        let Some(completion) = active.job.try_take_completion() else {
            return Ok(());
        };
        let active = self.acquisition.take().ok_or("Retained original acquisition disappeared")?;
        let failure = match completion.outcome {
            JobOutcome::Success(()) => return Ok(()),
            JobOutcome::Failure(cause) => {
                tracing::warn!(operation="workspace_original_acquisition",digest=%active.asset.digest,cause=%cause,"Retained original verification failed");
                SessionMediaFailure::Unavailable
            }
            outcome => {
                tracing::warn!(operation="workspace_original_acquisition",digest=%active.asset.digest,outcome=?outcome.kind(),"Retained original verification stopped");
                SessionMediaFailure::Cancelled
            }
        };
        if self.retiring {
            return Ok(());
        }
        let mut index = 0;
        while index < self.pending.len() {
            let request = &self.pending[index];
            let matches = driver
                .and_then(|driver| {
                    driver
                        .asset(request.output_id(), request.binding())
                        .or_else(|| driver.pending_asset(request.output_id(), request.binding()))
                })
                .is_some_and(|asset| asset == active.asset);
            if matches {
                let request = self.pending.remove(index).ok_or("Retained request disappeared")?;
                commands.push_back(DesktopStartupCommand::SendSessionMedia(
                    SessionMediaReply::unavailable(
                        request.identity(),
                        request.request_id(),
                        failure,
                    ),
                ));
            } else {
                index += 1;
            }
        }
        Ok(())
    }
}
