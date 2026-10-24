// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain fresh reload source/HTTP custody separately from the unchanged shared backend.
use super::*;
#[path = "frontend_refresh/source_close.rs"]
mod source_close;
use crate::production::coordinator_backend::BackendResources;
use source_close::SourceClose;
use sugarsubstitute_application::startup::{
    ConnectionBudget, ConnectionBudgetKind, FrontendBudget,
};
use sugarsubstitute_comfy::{
    readiness_campaign::{CampaignStep, ReadinessCampaign},
    readiness_transport::{READINESS_TRANSPORT_BYTES, VerifiedReadiness},
};
use sugarsubstitute_contracts::lifecycle::BackendTarget;
use sugarsubstitute_contracts::{
    execution::{JobClass, JobOutcome, Operation, OwnedJob},
    lifecycle::{LeaseKey, ShutdownBudget},
    startup::MainIdentity,
};
use sugarsubstitute_supervision::installed_frontend::{
    FRONTEND_SOURCE_BYTES, FRONTEND_SOURCE_CLOSE_BYTES, FrontendSourceError,
    InstalledFrontendLease, InstalledFrontendOwner,
};

/// Preserve both original cleanup failures when independent retirements reject together.
#[derive(Debug)]
struct CleanupFailure {
    original: Cause,
    independent: Cause,
}
impl std::fmt::Display for CleanupFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Frontend retirement failed: {}; independent retirement failed: {}",
            self.original, self.independent
        )
    }
}
impl Error for CleanupFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.original.as_ref())
    }
}

fn combine_retirement(original: Result<()>, independent: Result<()>) -> Result<()> {
    match (original, independent) {
        (Err(original), Err(independent)) => {
            Err(Box::new(CleanupFailure { original, independent }))
        }
        (Err(cause), _) | (_, Err(cause)) => Err(cause),
        (Ok(()), Ok(())) => Ok(()),
    }
}

pub(super) struct FrontendRefresh {
    index: usize,
    identity: MainIdentity,
    lease: LeaseKey,
    until: Moment,
    source: InstalledFrontendOwner,
    inspecting: bool,
    source_failure: Option<FrontendSourceError>,
    cancelled_reply: bool,
    source_lease: Option<InstalledFrontendLease>,
    source_close: SourceClose,
    campaign: ReadinessCampaign,
    proof: Option<VerifiedReadiness>,
    published: bool,
    shutdown: Option<ShutdownBudget>,
    pub(super) absent_ack: Option<usize>,
}
impl FrontendRefresh {
    pub(super) fn retired(&self) -> bool {
        self.source_close.closed() && self.campaign.settled()
    }
    pub(super) fn deadline(&self) -> Option<Moment> {
        if self.retired() {
            None
        } else if let Some(shutdown) = self.shutdown {
            Some(shutdown.settle_until)
        } else if self.published {
            None
        } else if self.inspecting {
            Some(self.until)
        } else {
            self.campaign.next_deadline()
        }
    }
    pub(super) fn close(&mut self, shutdown: ShutdownBudget) {
        let first = *self.shutdown.get_or_insert(shutdown);
        self.source.request_cancel();
        self.campaign.cancel(first);
        self.proof = None;
    }
    pub(super) fn proof(
        &self,
        identity: MainIdentity,
        lease: LeaseKey,
    ) -> Result<&VerifiedReadiness> {
        if self.identity != identity || self.lease != lease || self.shutdown.is_some() {
            return Err(ParentError::ForeignPeer.into());
        }
        let proof = self.proof.as_ref().ok_or(ParentError::WrongPhase)?;
        proof.validate_current()?;
        Ok(proof)
    }
    fn spec(resources: &BackendResources<'_>, bytes: u64, until: Moment) -> Result<JobSpec> {
        Ok(JobSpec {
            id: resources.jobs.allocate()?,
            operation: Operation::Readiness,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    /// Admit cleanup immediately on the actual closing edge; pending native work wakes its own owner.
    pub(super) fn schedule_cleanup(
        &mut self,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        let now = resources.execution.client.now()?;
        if !self.inspecting && self.source_close.needs_submission() {
            let spec = Self::spec(
                resources.execution,
                FRONTEND_SOURCE_CLOSE_BYTES,
                self.shutdown.ok_or(ParentError::WrongPhase)?.settle_until,
            )?;
            self.source_close.submit(
                &self.source,
                resources.execution.client,
                resources.execution.cleanup,
                spec,
            )?;
        }
        // Return the source close's fixed credit before admitting HTTP retirement.
        if self.source_close.awaiting_source() {
            return Ok(());
        }
        if !self.campaign.settled() {
            let spec = Self::spec(
                resources.execution,
                READINESS_TRANSPORT_BYTES,
                self.campaign.cleanup_deadline(now)?,
            )?;
            match self.campaign.submit_cleanup(
                resources.execution.client,
                resources.execution.cleanup,
                spec,
            ) {
                Ok(_) => (),
                Err(sugarsubstitute_comfy::readiness_campaign::CampaignError::Rejected(
                    sugarsubstitute_comfy::readiness_campaign::CampaignRejection::WrongPhase,
                )) => (),
                Err(cause) => return Err(cause.into()),
            }
        }
        Ok(())
    }
    fn progress(
        &mut self,
        batch: &CompletionBatch,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<bool> {
        let now = resources.execution.client.now()?;
        if self.inspecting {
            match self.source.take_inspection() {
                Ok(Some(source)) => {
                    if self.shutdown.is_none() {
                        // Revalidate the retained backend/source/listener before lifting preparation expiry.
                        let _input = resources.backend.frontend_reload_probe(
                            self.lease.resource,
                            source.clone(),
                            resources.execution,
                        )?;
                        self.campaign.admit_readiness_wait(resources.finite, now)?;
                    }
                    self.source_lease = Some(source);
                    self.inspecting = false;
                }
                Ok(None) => (),
                Err(cause) => {
                    self.inspecting = false;
                    self.source_failure = Some(cause.clone());
                    if self.shutdown.is_none()
                        || !matches!(
                            cause.terminal_kind(),
                            Some(TerminalKind::Cancelled | TerminalKind::TimedOut)
                        )
                    {
                        return Err(cause.into());
                    }
                }
            }
        }
        let step = self.campaign.consume(batch, now);
        if self.shutdown.is_some() {
            let source_result = self.source_close.consume();
            let cleanup_result = self.schedule_cleanup(resources);
            combine_retirement(
                combine_retirement(step.map(|_| ()).map_err(Into::into), source_result),
                cleanup_result,
            )?;
            return Ok(false);
        }
        let step = step?;
        if self.published {
            return Ok(false);
        }
        if self.inspecting && now >= self.until {
            return Err(ParentError::WrongPhase.into());
        }
        if step == CampaignStep::Ready {
            self.proof = self.campaign.take_readiness();
            let proof = self.proof(self.identity, self.lease)?;
            if proof.managed_resource()
                != matches!(resources.target, BackendTarget::Managed(_))
                    .then_some(self.lease.resource)
            {
                return Err(ParentError::ForeignPeer.into());
            }
            return Ok(true);
        }
        if step == CampaignStep::Failed
            && let Some(cause) = self.campaign.take_failure()
        {
            return Err(cause.into());
        }
        if self.campaign.needs_continuation(now)? {
            let spec = Self::spec(
                resources.execution,
                READINESS_TRANSPORT_BYTES,
                self.campaign.probe_until(now)?,
            )?;
            self.campaign.submit_continuation(
                resources.execution.client,
                resources.finite,
                spec,
            )?;
        } else if let Some(source) = &self.source_lease
            && self.campaign.needs_input(now)?
        {
            let input = resources.backend.frontend_reload_probe(
                self.lease.resource,
                source.clone(),
                resources.execution,
            )?;
            let spec = Self::spec(
                resources.execution,
                READINESS_TRANSPORT_BYTES,
                self.campaign.probe_until(now)?,
            )?;
            self.campaign.submit_attempt(
                resources.execution.client,
                resources.finite,
                spec,
                input,
            )?;
        }
        Ok(false)
    }
}
impl ParentLifecycle {
    pub(super) fn begin_frontend_refresh(
        &mut self,
        index: usize,
        identity: MainIdentity,
        lease: LeaseKey,
        until: Moment,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        if self.frontend_refresh.is_some()
            || self.main.stage()
                != sugarsubstitute_application::supervision::MainAuthorizationStage::Retired
        {
            return Err(ParentError::WrongPhase.into());
        }
        let (source, target) = resources.backend.frontend_reload_source(lease.resource)?;
        let now = resources.execution.client.now()?;
        let kind = match resources.target {
            BackendTarget::Managed(_) => ConnectionBudgetKind::Managed,
            BackendTarget::Attached(_) => ConnectionBudgetKind::Attached,
        };
        let budget = ConnectionBudget::from_until(kind, now, until)?;
        let campaign = ReadinessCampaign::new(target, budget)?;
        let spec = FrontendRefresh::spec(resources.execution, FRONTEND_SOURCE_BYTES, until)?;
        let mut source = source;
        source.submit_inspection(resources.execution.client, resources.finite, spec)?;
        self.requests.defer_reply(index)?;
        self.frontend_refresh = Some(FrontendRefresh {
            index,
            identity,
            lease,
            until,
            source,
            inspecting: true,
            source_failure: None,
            cancelled_reply: false,
            source_lease: None,
            source_close: SourceClose::default(),
            campaign,
            proof: None,
            published: false,
            shutdown: None,
            absent_ack: None,
        });
        Ok(())
    }
    pub(super) fn progress_frontend_refresh(
        &mut self,
        batch: &CompletionBatch,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        let Some(refresh) = &mut self.frontend_refresh else {
            return Ok(());
        };
        if self.closed || self.cleanup.is_some() {
            refresh.close(
                self.cleanup.unwrap_or(ShutdownBudget::new(resources.execution.client.now()?)?),
            );
        }
        if refresh.shutdown.is_some() && !refresh.published && !refresh.cancelled_reply {
            self.requests.get_mut(refresh.index)?.reply = Some(Reply::Cancelled);
            refresh.cancelled_reply = true;
        }
        let ready = refresh.progress(batch, resources)?;
        if refresh.retired()
            && let Some(index) = refresh.absent_ack
        {
            self.requests.get_mut(index)?.reply = Some(Reply::Acknowledged);
            self.frontend_refresh = None;
            return Ok(());
        }
        if !ready {
            return Ok(());
        }
        let (index, identity, lease) = (refresh.index, refresh.identity, refresh.lease);
        let proof = refresh.proof(identity, lease)?;
        let origin =
            resources.backend.ready_origin(lease.resource)?.ok_or(ParentError::WrongPhase)?;
        let manifest = proof.source().manifest();
        let document = recovery::MountedDocument::new(
            origin,
            manifest.frontend_version(),
            manifest.cube_version(),
        )?;
        let frontend = FrontendBudget::new(resources.execution.client.now()?)?;
        self.main.begin(identity, lease, frontend.until())?;
        resources.profile.begin_main_attempt(identity)?;
        self.folders.begin(identity);
        self.lease = Some(lease);
        self.recovery.running.as_mut().ok_or(ParentError::WrongPhase)?.document = document;
        self.requests.get_mut(index)?.reply = Some(Reply::Reloaded { lease, identity });
        self.frontend_refresh.as_mut().ok_or(ParentError::WrongPhase)?.published = true;
        Ok(())
    }
}
