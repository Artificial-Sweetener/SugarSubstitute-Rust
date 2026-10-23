// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain authenticated phase custody outside execution results, including cancelled results.
use super::*;
use crate::{
    setup_storage::{
        AuthenticatedOffer, CachedCandidate, Download, InstallationSession, TrustStore,
        UpdaterPackage, VerifiedDownload,
    },
    update_repository::{Check, Failure, Preparation, Repository},
};
use std::{io, sync::Arc};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_contracts::update::Component;

/// A retained boundary whose values keep the original storage exclusions and byte reservations.
pub(super) enum Phase {
    Open,
    Discover(Box<TrustStore>),
    SelectEngine(Box<TrustStore>, u64),
    DiscoverEngine(Box<TrustStore>, u64),
    Restore(Box<TrustStore>),
    Reopen(Box<TrustStore>, Box<AuthenticatedOffer>),
    RestoreArchive(Box<TrustStore>, Box<AuthenticatedOffer>),
    Download(Box<TrustStore>, Box<AuthenticatedOffer>),
    Import(Box<TrustStore>, Box<AuthenticatedOffer>, Box<VerifiedDownload>),
    ImportEngine(Box<TrustStore>, Box<AuthenticatedOffer>, Box<VerifiedDownload>),
    Ready(Box<SignedCandidate>),
    EngineReady(EngineUpdate),
    Empty,
}

/// Synchronous operation state retained independently of execution-result delivery.
pub(super) struct State {
    pub(super) installation: PathBuf,
    pub(super) workspace: PathBuf,
    pub(super) repository_id: [u8; 32],
    pub(super) subscription: Option<Subscription>,
    pub(super) cached: bool,
    pub(super) repository: Repository,
    pub(super) phase: Phase,
    pub(super) progress: Progress,
    pub(super) failure: Option<Arc<Failure>>,
    pub(super) retirement_failure: Option<Arc<Failure>>,
}
impl State {
    /// Advance a single admitted boundary. Failed custody retires on the same worker where possible.
    pub(super) fn advance(
        &mut self,
        now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<(), Arc<Failure>> {
        self.step(now, context).map_err(|cause| {
            let cause = Arc::new(cause);
            self.failure = Some(cause.clone());
            self.progress = Progress::Failed;
            cause
        })
    }

    /// Consume one phase; an interrupted phase never masquerades as a completed candidate.
    fn step(&mut self, mut now: impl FnMut() -> i64, context: &JobContext) -> Result<(), Failure> {
        let phase = std::mem::replace(&mut self.phase, Phase::Empty);
        match &phase {
            Phase::Reopen(trust, _)
            | Phase::RestoreArchive(trust, _)
            | Phase::Download(trust, _)
            | Phase::Import(trust, _, _)
            | Phase::ImportEngine(trust, _, _)
            | Phase::SelectEngine(trust, _)
            | Phase::DiscoverEngine(trust, _) => {
                self.channel(trust, context)?;
            }
            _ => {}
        }
        let (next, progress) = match phase {
            Phase::Open => {
                let mut trust =
                    Box::new(TrustStore::open(&self.installation, self.repository_id, context)?);
                self.subscription = Some(trust.subscription(context)?);
                match trust.acquire_preparation(context) {
                    Err(crate::setup_storage::TrustFailure::Storage(cause))
                        if cause.kind() == io::ErrorKind::WouldBlock =>
                    {
                        (Phase::Empty, Progress::AlreadyPreparing)
                    }
                    Err(cause) => return Err(cause.into()),
                    Ok(()) if self.cached => (Phase::Restore(trust), Progress::Restoring),
                    Ok(()) => (Phase::Discover(trust), Progress::Checking),
                }
            }
            Phase::Restore(trust) => {
                let request = trust.application_request(self.channel(&trust, context)?, context)?;
                match trust.cached_offer(request, now(), context)? {
                    None => (Phase::Empty, Progress::Unavailable),
                    Some(offer) if offer.offer().admission() != Admission::Prepare => {
                        (Phase::Empty, Progress::Decision(offer.offer().admission()))
                    }
                    Some(offer) => (Phase::Reopen(trust, Box::new(offer)), Progress::Reopening),
                }
            }
            Phase::Reopen(trust, offer) => {
                match SignedCandidate::reopen_cached(
                    &self.installation,
                    *trust,
                    *offer,
                    &mut now,
                    context,
                )? {
                    CachedCandidate::Ready(candidate) => (Phase::Ready(candidate), Progress::Ready),
                    CachedCandidate::Missing(trust, offer) => {
                        (Phase::RestoreArchive(trust, offer), Progress::RestoringArchive)
                    }
                }
            }
            Phase::RestoreArchive(trust, offer) => {
                let authorization = offer.authorization(&trust);
                let target = trust
                    .offer(
                        authorization.metadata,
                        authorization.request,
                        authorization.descriptor,
                        now(),
                        context,
                    )?
                    .into_package()?;
                let total = target.length();
                let download = Download::open(trust.runtime_cache(context)?, target, context)?;
                if download.offset()? != total {
                    (Phase::Empty, Progress::Unavailable)
                } else {
                    let download = download.finish(&mut now, context)?;
                    (Phase::Import(trust, offer, Box::new(download)), Progress::Preparing)
                }
            }
            Phase::Discover(trust) => {
                let request = trust.application_request(self.channel(&trust, context)?, context)?;
                match self.repository.discover(&trust, request, &mut now, context)? {
                    Check::MoreRoots => (Phase::Discover(trust), Progress::Checking),
                    Check::Offer(offer) => {
                        let admission = offer.offer().admission();
                        if admission == Admission::Prepare {
                            let total = offer.offer().offer().package_bytes();
                            (
                                Phase::Download(trust, offer),
                                Progress::Downloading { received: 0, total },
                            )
                        } else if admission == Admission::UpdateEngine {
                            (
                                Phase::SelectEngine(
                                    trust,
                                    offer.offer().offer().minimum_updater_release(),
                                ),
                                Progress::SelectingEngine,
                            )
                        } else {
                            (Phase::Empty, Progress::Decision(admission))
                        }
                    }
                }
            }
            Phase::SelectEngine(trust, minimum) => {
                match trust.pin_updater(self.channel(&trust, context)?, minimum, context)? {
                    Some(image) => (
                        Phase::EngineReady(EngineUpdate::Accepted(Box::new(image))),
                        Progress::EngineReady,
                    ),
                    None => (Phase::DiscoverEngine(trust, minimum), Progress::CheckingEngine),
                }
            }
            Phase::DiscoverEngine(trust, minimum) => {
                let request = trust.updater_request(self.channel(&trust, context)?, context)?;
                match self.repository.discover(&trust, request, &mut now, context)? {
                    Check::MoreRoots => {
                        (Phase::DiscoverEngine(trust, minimum), Progress::CheckingEngine)
                    }
                    Check::Offer(offer) => {
                        if offer.offer().offer().sequence() < minimum {
                            return Err(io::Error::other("Published updater does not meet the application's required release").into());
                        }
                        match offer.offer().admission() {
                            Admission::Prepare => {
                                let total = offer.offer().offer().package_bytes();
                                (
                                    Phase::Download(trust, offer),
                                    Progress::Downloading { received: 0, total },
                                )
                            }
                            Admission::Current => {
                                (Phase::SelectEngine(trust, minimum), Progress::SelectingEngine)
                            }
                            Admission::Installer | Admission::UpdateEngine => {
                                (Phase::Empty, Progress::Decision(Admission::Installer))
                            }
                        }
                    }
                }
            }
            Phase::Download(trust, offer) => match self.repository.prepare(
                &offer,
                &trust,
                &self.installation,
                &mut now,
                context,
            )? {
                Preparation::Pending { received, total } => {
                    (Phase::Download(trust, offer), Progress::Downloading { received, total })
                }
                Preparation::Complete(download) => {
                    if offer.authorization(&trust).request.component == Component::Updater {
                        (Phase::ImportEngine(trust, offer, download), Progress::PreparingEngine)
                    } else {
                        (Phase::Import(trust, offer, download), Progress::Preparing)
                    }
                }
            },
            Phase::ImportEngine(trust, offer, download) => {
                let prepared = UpdaterPackage::import(
                    &self.installation,
                    *trust,
                    *offer,
                    *download,
                    &self.workspace,
                    &mut now,
                    context,
                )?
                .stage(&mut now, context)?;
                (
                    Phase::EngineReady(EngineUpdate::Prepared(Box::new(prepared))),
                    Progress::EngineReady,
                )
            }
            Phase::Import(trust, offer, download) => {
                // One reservation funds both retained payloads and transient trust operations.
                // Splitting these moves across jobs retains each old peak until candidate disposal.
                let imported = InstallationSession::maintain(&self.installation, context)?
                    .import_download(
                        *trust,
                        *offer,
                        *download,
                        &self.workspace,
                        &mut now,
                        context,
                    )?;
                let prepared = imported.prepare(&mut now, context)?;
                let runtime = prepared
                    .maintenance_runtime()
                    .ok_or_else(|| {
                        io::Error::other(
                            "Update preparation requires an existing ComfyUI configuration",
                        )
                    })?
                    .clone();
                // Completion owns staging and destination verification under current authority.
                let candidate = prepared.complete(&runtime, &mut now, context)?;
                candidate.save_restart_evidence(context)?;
                (Phase::Ready(Box::new(candidate)), Progress::Ready)
            }
            other => {
                self.phase = other;
                return Err(io::Error::other("Update preparation has no advanceable phase").into());
            }
        };
        self.phase = next;
        self.progress = progress;
        Ok(())
    }

    /// Close HTTP custody before dropping storage owners on this admitted cleanup worker.
    pub(super) fn retire(&mut self) -> Result<(), Failure> {
        self.repository.retire()?;
        self.phase = Phase::Empty;
        self.progress = Progress::Closed;
        self.retirement_failure = None;
        Ok(())
    }

    /// Discovery and cache restoration use the saved local revision, never a helper argument.
    fn channel(
        &self,
        trust: &TrustStore,
        context: &JobContext,
    ) -> Result<sugarsubstitute_contracts::update::Channel, Failure> {
        let current = trust.subscription(context)?;
        if self.subscription != Some(current) {
            return Err(io::Error::other("Update subscription changed during discovery").into());
        }
        Ok(current.channel())
    }
}
