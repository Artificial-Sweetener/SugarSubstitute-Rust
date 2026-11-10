// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serve bounded retained originals through fresh native request and worker custody.
mod acquisition;
mod reader;
use super::*;
use std::{collections::BTreeMap, num::NonZeroU64};
use sugarsubstitute_desktop::{SessionMediaFailure, SessionMediaObservation, SessionMediaReply};
use sugarsubstitute_supervision::RetainedArtifactReader;

/// Borrow the existing execution owner and original retirement cutoff for this pump turn.
#[derive(Clone, Copy)]
pub(super) struct MediaAdmission<'a> {
    pub(super) client: &'a ExecutionClient,
    pub(super) scope: &'a Scope,
    pub(super) jobs: &'a JobSequence,
    pub(super) now: Moment,
    pub(super) cutoff: Option<Moment>,
}
/// Observe current host-owned workspace authority without retaining it across pump turns.
pub(super) struct MediaSource<'a> {
    pub(super) driver: Option<&'a WorkspaceDriver>,
    pub(super) store: Option<&'a WorkspaceStore>,
    pub(super) resident: Option<&'a MemoryLease>,
    pub(super) identity: Option<MainIdentity>,
}
struct Verified {
    reader: RetainedArtifactReader,
    asset: SessionAsset,
    animated: bool,
    verified: bool,
    touched: Moment,
}
const LARGE_ORIGINAL_BYTES: u64 = 4 * sugarsubstitute_desktop::MAX_MEDIA_BYTES as u64;
struct MediaRead {
    mime: &'static str,
    total: u64,
    start: Option<u64>,
    bytes: Vec<u8>,
    memory: Option<MemoryClaim>,
    animated: bool,
    head: bool,
}
struct Running {
    observed: SessionMediaObservation,
    expected: SessionAsset,
    until: Moment,
    job: OwnedJob<MediaRead, StoreError>,
}
pub(super) struct MediaWork {
    pending: VecDeque<SessionMediaObservation>,
    running: Vec<Running>,
    readers: Arc<Mutex<BTreeMap<String, Verified>>>,
    close: Option<OwnedJob<(), StoreError>>,
    acquisition: Option<acquisition::Acquiring>,
    retiring: bool,
}
impl MediaWork {
    pub(super) fn new() -> Self {
        Self {
            pending: VecDeque::new(),
            running: vec![],
            readers: Arc::new(Mutex::new(BTreeMap::new())),
            close: None,
            acquisition: None,
            retiring: false,
        }
    }
    pub(super) fn offer(&mut self, request: SessionMediaObservation) -> Result<(), Box<dyn Error>> {
        if self.retiring || self.pending.len() >= 8 {
            return Err("Retained media request admission saturated".into());
        }
        if request.head() {
            let position =
                self.pending.iter().position(|queued| !queued.head()).unwrap_or(self.pending.len());
            self.pending.insert(position, request);
        } else {
            self.pending.push_back(request);
        }
        Ok(())
    }
    pub(super) fn cancel(&mut self, identity: MainIdentity, request: NonZeroU64) {
        self.pending
            .retain(|pending| pending.identity() != identity || pending.request_id() != request);
        for active in &self.running {
            if active.observed.identity() == identity && active.observed.request_id() == request {
                active.job.cancel();
            }
        }
    }
    pub(super) fn retire(&mut self, identity: MainIdentity) {
        self.retiring = true;
        self.pending.retain(|request| request.identity() != identity);
        if let Some(active) = &self.acquisition {
            active.job.cancel();
        }
        for active in &self.running {
            if active.observed.identity() == identity {
                active.job.cancel();
            }
        }
    }
    pub(super) fn advance(
        &mut self,
        admission: MediaAdmission<'_>,
        source: MediaSource<'_>,
        now_native: Instant,
        commands: &mut VecDeque<DesktopStartupCommand>,
    ) -> Result<(), Box<dyn Error>> {
        let MediaAdmission { client, scope, jobs, now, cutoff } = admission;
        let MediaSource { driver, store, resident, identity } = source;
        let mut index = 0;
        while index < self.running.len() {
            let active = &mut self.running[index];
            if now >= active.until {
                active.job.cancel();
            }
            let Some(completion) = active.job.try_take_completion() else {
                index += 1;
                continue;
            };
            let active = self.running.swap_remove(index);
            if self.retiring {
                if let JobOutcome::Failure(cause) = completion.outcome {
                    tracing::warn!(operation="workspace_media_retirement",request_id=active.observed.request_id().get(),cause=%cause,"Retired media worker failed; result withheld");
                }
                continue;
            }
            let current = identity == Some(active.observed.identity())
                && driver.is_some_and(|driver| {
                    driver.asset_valid_for_delivery(
                        active.observed.webview(),
                        active.observed.output_id(),
                        active.observed.binding(),
                        &active.expected,
                    )
                });
            if !current && let JobOutcome::Failure(cause) = &completion.outcome {
                tracing::warn!(operation="workspace_stale_media_terminal",request_id=active.observed.request_id().get(),cause=%cause,"Stale retained media worker failed; pixels withheld");
            }
            let reply = match completion.outcome {
                _ if !current => SessionMediaReply::unavailable(
                    active.observed.identity(),
                    active.observed.request_id(),
                    SessionMediaFailure::Stale,
                ),
                JobOutcome::Success(read) if now < active.until => {
                    tracing::debug!(operation="workspace_media_delivered",request_id=active.observed.request_id().get(),output_id=%active.observed.output_id(),generation=active.observed.binding().generation,head=read.head,total_bytes=read.total,response_bytes=read.bytes.len(),animated=read.animated,"Exact retained media response admitted after worker completion");
                    if read.head {
                        SessionMediaReply::metadata(
                            active.observed.identity(),
                            active.observed.request_id(),
                            read.mime,
                            read.total,
                            read.animated,
                        )?
                    } else {
                        let memory =
                            read.memory.ok_or("Retained media body has no admission claim")?;
                        match read.start {
                            Some(start) => SessionMediaReply::partial(
                                active.observed.identity(),
                                active.observed.request_id(),
                                read.mime,
                                read.bytes,
                                memory,
                                start,
                                read.total,
                            )?,
                            None => SessionMediaReply::new(
                                active.observed.identity(),
                                active.observed.request_id(),
                                read.mime,
                                read.bytes,
                                memory,
                            )?,
                        }
                    }
                }
                JobOutcome::Failure(cause) => {
                    tracing::warn!(operation="workspace_media_read",request_id=active.observed.request_id().get(),cause=%cause,"Retained media unavailable");
                    SessionMediaReply::unavailable(
                        active.observed.identity(),
                        active.observed.request_id(),
                        SessionMediaFailure::Unavailable,
                    )
                }
                _ => SessionMediaReply::unavailable(
                    active.observed.identity(),
                    active.observed.request_id(),
                    SessionMediaFailure::Cancelled,
                ),
            };
            commands.push_back(DesktopStartupCommand::SendSessionMedia(reply));
        }
        self.observe_acquisition(driver, commands)?;
        if self.retiring {
            self.close_readers(client, scope, jobs, cutoff)?;
            return Ok(());
        }
        if self.acquisition.is_some() {
            return Ok(());
        }
        while self.running.is_empty() && commands.len() < 8 {
            let Some(request) = self.pending.front() else {
                break;
            };
            let valid = identity == Some(request.identity())
                && driver.is_some_and(|driver| driver.webview() == request.webview());
            if !valid || request.deadline() <= now_native {
                let request = self.pending.pop_front().ok_or("Media request disappeared")?;
                commands.push_back(DesktopStartupCommand::SendSessionMedia(
                    SessionMediaReply::unavailable(
                        request.identity(),
                        request.request_id(),
                        if valid {
                            SessionMediaFailure::Cancelled
                        } else {
                            SessionMediaFailure::Stale
                        },
                    ),
                ));
                continue;
            }
            let Some(asset) = driver.and_then(|driver| {
                driver
                    .asset(request.output_id(), request.binding())
                    .or_else(|| driver.pending_asset(request.output_id(), request.binding()))
            }) else {
                // A materializing page can issue HEAD before its acknowledgement is delivered.
                // Preserve this exact bounded request and its original cutoff while the binding settles.
                break;
            };
            let store = store.ok_or("Retained media store absent")?;
            let until = now.checked_add(request.deadline().duration_since(now_native))?;
            let until = cutoff.map_or(until, |cutoff| cutoff.min(until));
            let path = store.asset_path(&asset)?;
            let resident = resident.ok_or("Retained media native allocation absent")?.clone();
            if !reader::prepared(&self.readers, &asset)? {
                self.acquire(admission, path, asset, resident)?;
                break;
            }
            let readers = self.readers.clone();
            let head = request.head();
            let range = request.range();
            let expected = asset.clone();
            let spec = JobSpec {
                id: jobs.allocate()?,
                operation: Operation::Infrastructure,
                class: JobClass::InfrastructureOrdinary,
                deadline: Some(until),
                reservation_bytes: sugarsubstitute_desktop::MAX_MEDIA_BYTES as u64 + 128 * 1024,
                replacement: None,
            };
            let job = match client.submit_blocking(scope, spec, move |context| {
                reader::read(&readers, &asset, range, head, &context)
            }) {
                Ok(job) => job,
                Err(
                    AdmissionError::MemorySaturated { .. }
                    | AdmissionError::QueueSaturated { .. }
                    | AdmissionError::CompletionSaturated,
                ) => break,
                Err(cause) => return Err(cause.into()),
            };
            let request = self.pending.pop_front().ok_or("Media request disappeared")?;
            tracing::debug!(operation="workspace_media_admitted",request_id=request.request_id().get(),output_id=%request.output_id(),runtime_session=%request.binding().runtime_session,generation=request.binding().generation,head=request.head(),pending_restore=driver.is_some_and(|driver| driver.asset(request.output_id(),request.binding()).is_none()),digest=%expected.digest,total_bytes=expected.bytes,"Retained media request admitted against current checkpoint authority");
            self.running.push(Running { observed: request, expected, until, job });
        }
        Ok(())
    }
    fn close_readers(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        cutoff: Option<Moment>,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(job) = &mut self.close {
            if let Some(completion) = job.try_take_completion() {
                self.close = None;
                match completion.outcome {
                    JobOutcome::Success(()) => {}
                    JobOutcome::Failure(cause) => return Err(cause.into()),
                    outcome => {
                        return Err(std::io::Error::other(format!(
                            "Retained media retirement stopped: {:?}",
                            outcome.kind()
                        ))
                        .into());
                    }
                }
            }
            return Ok(());
        }
        if !self.running.is_empty() || self.acquisition.is_some() {
            return Ok(());
        }
        if self
            .readers
            .try_lock()
            .map_err(|_| "Retained media registry unexpectedly borrowed after workers settled")?
            .is_empty()
        {
            return Ok(());
        }
        let readers = self.readers.clone();
        // Closing readers ends on its observed outcome; only the shutdown cutoff bounds it.
        let spec = JobSpec {
            id: jobs.allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(cutoff.unwrap_or(crate::job_deadline::OPEN_ENDED)),
            reservation_bytes: 128 * 1024,
            replacement: None,
        };
        self.close = Some(client.submit_blocking(scope, spec, move |context| {
            let mut readers = readers.lock().map_err(|_| StoreError::Changed)?;
            for verified in readers.values_mut() {
                verified.reader.close(&context)?;
            }
            readers.clear();
            Ok(())
        })?);
        Ok(())
    }
    pub(super) fn settled(&self) -> bool {
        self.pending.is_empty()
            && self.running.is_empty()
            && self.close.is_none()
            && self.acquisition.is_none()
            && self.readers.try_lock().is_ok_and(|readers| readers.is_empty())
    }
}
