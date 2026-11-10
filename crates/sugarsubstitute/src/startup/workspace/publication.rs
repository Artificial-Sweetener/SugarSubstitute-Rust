// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish only after native archive receipts and every temporary final's exact original exist.
use super::*;
use crate::input_documents::{InputDocumentStore, workspace::WorkspaceInputs};
use sugarsubstitute_comfy::output_media::{
    ComfyOutputTransfer, ComfyViewTarget, OutputArtifactIdentity,
};
use sugarsubstitute_contracts::canvas_surface::CanvasFileReference;
use sugarsubstitute_execution::OwnedInputError;
use sugarsubstitute_supervision::cache::{CachePolicy, RuntimeCache, utc_seconds};

pub(super) struct Input {
    ticket: CaptureTicket,
    captured: workspace_session::CapturedWorkspace,
    native: WorkspaceInputs,
    backend: WorkspaceContext,
    baseline: workspace_session::CheckpointBaseline,
}
impl WorkspaceWork {
    pub(super) fn wants_inputs(&self) -> bool {
        self.capture.is_some() && self.publish_input.is_none()
    }
    pub(super) fn offer_inputs(
        &mut self,
        native: WorkspaceInputs,
        backend: WorkspaceContext,
    ) -> Result<(), Box<dyn Error>> {
        let expected = self.context.as_ref().ok_or("Workspace context absent")?;
        if backend.identity != expected.identity
            || backend.backend != expected.backend
            || backend.installation != expected.installation
            || backend.profile != expected.profile
        {
            return Err("Workspace backend changed before publication".into());
        }
        let (ticket, captured) =
            self.capture.take().ok_or("Workspace input snapshot was not requested")?;
        let driver = self.driver.as_mut().ok_or("Workspace driver absent")?;
        driver.publishing(&ticket, &captured)?;
        let baseline = driver.baseline();
        self.publish_input = Some(Input { ticket, captured, native, backend, baseline });
        Ok(())
    }
    pub(super) fn input_changed(
        &mut self,
        generation: Option<u64>,
        now: Moment,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(generation) = generation {
            let changed = self
                .input_generation
                .replace(generation)
                .is_some_and(|previous| previous != generation);
            if changed && let Some(driver) = &mut self.driver {
                driver.native_changed(now)?;
            }
        }
        Ok(())
    }
    pub(super) fn submit_publication(&mut self, now: Moment) -> Result<(), Box<dyn Error>> {
        self.retire_transfers(now)?;
        if self.active.is_some()
            || self.publish_input.is_none()
            || self.retirement_job.is_some()
            || !self
                .retirements
                .try_lock()
                .map_err(|_| "Workspace transfer retirement is borrowed")?
                .is_empty()
        {
            return Ok(());
        }
        let input = self.publish_input.as_ref().ok_or("Workspace publication input absent")?;
        let until = self
            .driver
            .as_ref()
            .and_then(WorkspaceDriver::deadline)
            .ok_or("Workspace publication cutoff absent")?;
        if now >= until {
            let input =
                self.publish_input.take().ok_or("Workspace publication input disappeared")?;
            self.driver
                .as_mut()
                .ok_or("Workspace driver absent")?
                .publication_failed(&input.ticket, now)?;
            return Ok(());
        }
        let bytes = WorkspaceStore::reservation_bytes()
            .checked_add(InputDocumentStore::save_reservation(&input.native.archive))
            .ok_or("Workspace publication admission overflow")?
            .checked_add(
                sugarsubstitute_comfy::output_media::OUTPUT_TRANSFER_BYTES + 8 * 1024 * 1024,
            )
            .ok_or("Workspace transfer admission overflow")?;
        let spec = self.job(bytes, Some(until))?;
        let ticket = input.ticket.clone();
        let directory =
            self.input_directory.clone().ok_or("Workspace native archive directory absent")?;
        let user = self.user.clone();
        let profile = self.profile.as_ref().ok_or("Workspace profile absent")?.clone();
        let retirements = self.retirements.clone();
        let client = self.client.as_ref().ok_or("Workspace execution absent")?;
        let scope = self.scope.as_ref().ok_or("Workspace scope absent")?;
        match client.submit_blocking_owned_input(scope,spec,&mut self.publish_input,move |mut input,context| {
            input.baseline.verify(&input.captured.checkpoint, &context)?;
            input.native.retain(&directory,&mut input.captured.checkpoint,&context).map_err(|cause| StoreError::Storage(std::io::Error::other(cause)))?;
            let mut profile = profile.lock().map_err(|_| StoreError::Changed)?;
            let cache = RuntimeCache::open(&user,CachePolicy::default(),&context)?;
            let known = input.baseline.retained_assets();
            let _known_memory = context.memory().claim((known.len() as u64).saturating_mul(128)).map_err(std::io::Error::other)?;
            let known: std::collections::BTreeMap<_,_> = known.iter().map(|asset| (&asset.digest, asset)).collect();
            for asset in &input.captured.checkpoint.assets {
                if known.get(&asset.digest).is_some_and(|prior| **prior == *asset) && profile.workspace.verify_asset(asset,&context)? { continue; }
                let output = input.captured.checkpoint.entries.iter().flat_map(|entry| &entry.outputs)
                    .find(|output| output.asset_digest.as_ref() == Some(&asset.digest)).ok_or(StoreError::Changed)?;
                if output.digest != asset.digest || output.bytes != asset.bytes { return Err(StoreError::Changed); }
                let reference = serde_json::to_vec(&serde_json::json!({"filename":output.file.filename,"subfolder":output.file.subfolder,"type":output.file.storage}))
                    .map_err(std::io::Error::other)?;
                let reference = CanvasFileReference::decode(&reference).map_err(std::io::Error::other)?;
                let transfer = Arc::new(ComfyOutputTransfer::verified(input.backend.origin,
                    ComfyViewTarget::artifact(&reference).map_err(std::io::Error::other)?,input.backend.peer,
                    sugarsubstitute_image_io::MAX_ENCODED_SOURCE_BYTES,
                    OutputArtifactIdentity::new(&asset.digest,asset.bytes).map_err(std::io::Error::other)?)
                    .map_err(std::io::Error::other)?);
                retirements.lock().map_err(|_| StoreError::Changed)?.push(transfer.clone());
                let result = transfer.fetch(cache.clone(),utc_seconds()?,&context).map_err(std::io::Error::other)
                    .map_err(StoreError::from).and_then(|mut reader| profile.retain_asset(asset,&mut reader,&context));
                if transfer.settled() {
                    retirements.lock().map_err(|_| StoreError::Changed)?.retain(|owner| !Arc::ptr_eq(owner,&transfer));
                }
                result?;
            }
            let receipt = profile.workspace.save_worker(&input.captured.checkpoint,&context)?;
            Ok(Published { captured: input.captured,receipt })
        }) {
            Ok(job) => self.active = Some(Active::Publish { ticket,job }),
            Err(OwnedInputError::Admission(AdmissionError::MemorySaturated {..} | AdmissionError::QueueSaturated {..} | AdmissionError::CompletionSaturated)) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
    fn retire_transfers(&mut self, now: Moment) -> Result<(), Box<dyn Error>> {
        if let Some(job) = &mut self.retirement_job {
            if let Some(completion) = job.try_take_completion() {
                self.retirement_job = None;
                match completion.outcome {
                    JobOutcome::Success(()) => {}
                    JobOutcome::Failure(cause) => return Err(cause.into()),
                    outcome => {
                        return Err(std::io::Error::other(format!(
                            "Workspace transfer retirement stopped: {:?}",
                            outcome.kind()
                        ))
                        .into());
                    }
                }
            }
            return Ok(());
        }
        if self.active.is_some()
            || self
                .retirements
                .try_lock()
                .map_err(|_| "Transfer retirement is borrowed")?
                .is_empty()
        {
            return Ok(());
        }
        let until = self.retirement_until.unwrap_or(now.checked_add(Duration::from_secs(15))?);
        let mut spec =
            self.job(sugarsubstitute_comfy::output_media::OUTPUT_TRANSFER_BYTES, Some(until))?;
        spec.class = JobClass::InfrastructureLifecycle;
        let retirements = self.retirements.clone();
        self.retirement_job =
            Some(self.client.as_ref().ok_or("Workspace execution absent")?.submit_blocking(
                self.scope.as_ref().ok_or("Workspace scope absent")?,
                spec,
                move |context| {
                    let mut retirements = retirements.lock().map_err(|_| StoreError::Changed)?;
                    for transfer in retirements.iter() {
                        transfer.retire(&context).map_err(std::io::Error::other)?;
                    }
                    retirements.retain(|transfer| !transfer.settled());
                    Ok(())
                },
            )?);
        Ok(())
    }
}
