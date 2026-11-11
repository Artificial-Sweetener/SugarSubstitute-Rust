// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain source launch custody until its sole native creator reports an observed disposition.
use super::*;
use std::{fs::File, os::windows::io::AsRawHandle, path::Path};

/// Source-side request state survives cancellation or discarded execution results.
pub(in crate::application_launch) struct Transfer {
    id: [u8; 32],
    published: bool,
    outcome: Option<CreationOutcome>,
    original: JobContext,
    pub(in crate::application_launch) closed: bool,
}
impl Transfer {
    /// Without a terminal reply the receiver may still be duplicating these exact source slots.
    pub(in crate::application_launch) fn pending(&self) -> bool {
        self.published && self.outcome.is_none()
    }
}

/// Authenticated launch data with its original native pins retained outside worker results.
/// Its frame must travel only over the private channel to the retained native launch creator.
/// Encoding is not publication, creation, application health or permission to accept an update.
/// Use in a dedicated, bounded updater process. Unanswered publication keeps native source
/// slots alive through process exit so owner loss cannot recycle them during receiver capture.
pub struct LaunchOffer {
    owner: ApplicationLaunch,
    id: [u8; 32],
    bytes: Vec<u8>,
    _memory: Arc<MemoryClaim>,
}
impl LaunchOffer {
    /// Mark creation uncertain before bytes can reach another process. An unanswered published
    /// offer must never restore an allegedly unstarted trial or cause a duplicate launch.
    /// # Errors
    /// Rejects expired/cancelled admission, retired custody and publication after a terminal reply.
    pub fn frame(&self, context: &JobContext) -> Result<&[u8], ClaimError> {
        admit(context)?;
        let mut state = crate::native_claims::lock(&self.owner.0);
        let transfer =
            state.transfer.as_mut().ok_or_else(|| ClaimError::invalid("launch_transfer_absent"))?;
        admit(&transfer.original)?;
        if transfer.original.owner_id() != context.owner_id() {
            return Err(ClaimError::invalid("launch_transfer_execution_owner"));
        }
        if transfer.closed || transfer.id != self.id || transfer.outcome.is_some() {
            return Err(ClaimError::invalid("launch_transfer_publication_closed"));
        }
        transfer.published = true;
        state.creation = CreationOutcome::Indeterminate;
        Ok(&self.bytes)
    }

    /// Observe a correlated response from the private creator channel before retiring this source.
    /// Only an actual `NotCreated` response restores unstarted-trial recovery eligibility.
    /// Repeated identical replies are idempotent; contradictory replies cannot alter the decision.
    /// # Errors
    /// Rejects malformed, foreign, premature, contradictory or late responses and expired admission.
    pub fn observe_result(
        &self,
        bytes: &[u8],
        context: &JobContext,
    ) -> Result<CreationOutcome, ClaimError> {
        admit(context)?;
        let outcome = wire::response(bytes, self.id)?;
        let mut state = crate::native_claims::lock(&self.owner.0);
        let transfer =
            state.transfer.as_mut().ok_or_else(|| ClaimError::invalid("launch_transfer_absent"))?;
        if transfer.original.owner_id() != context.owner_id() {
            return Err(ClaimError::invalid("launch_transfer_execution_owner"));
        }
        if transfer.id != self.id || !transfer.published {
            return Err(ClaimError::invalid("launch_transfer_reply_before_publication"));
        }
        if let Some(original) = transfer.outcome {
            return if original == outcome {
                Ok(original)
            } else {
                Err(ClaimError::invalid("launch_transfer_contradictory_reply"))
            };
        }
        if transfer.closed {
            return Err(ClaimError::invalid("launch_transfer_reply_after_retirement"));
        }
        transfer.outcome = Some(outcome);
        state.creation = outcome;
        Ok(outcome)
    }
}

impl ApplicationLaunch {
    /// Offer one already-verified installed or trial launch to its separate native creator.
    /// Reserve `TRANSFER_RESERVATION_BYTES` on an admitted worker. The source immediately closes
    /// local creation admission and retains its actual file/run custody through result observation.
    /// The caller retains this launch through `recover_and_retire`, even if the offer is discarded.
    /// # Errors
    /// Rejects unprepared or ordinary unverified commands, duplicate offers, invalid request identity,
    /// oversized handle inventories, allocation failures and expired/cancelled admission.
    pub fn offer(&self, id: [u8; 32], context: &JobContext) -> Result<LaunchOffer, ClaimError> {
        admit(context)?;
        let mut state = crate::native_claims::lock(&self.0);
        if state.issued || state.command.is_none() {
            return Err(ClaimError::invalid("launch_transfer_source_closed"));
        }
        let memory = Arc::new(
            context
                .memory()
                .claim(TRANSFER_RESERVATION_BYTES)
                .map_err(|cause| ClaimError::new("launch_transfer_memory", cause))?,
        );
        let request = match &state.custody {
            Some(LaunchCustody::Selected(selection)) => compose(
                id,
                &selection.application().executable(),
                selection.application().directory(),
                wire::Entry::Installed {
                    selection: selection.selection_handle().as_raw_handle() as usize as u64,
                },
                selection.launch_pins(),
            )?,
            Some(LaunchCustody::Trial(trial)) => {
                let transfer = trial.transfer()?;
                let [run, snapshot] =
                    transfer.handles().map(|handle| handle.as_raw_handle() as usize as u64);
                compose(
                    id,
                    &transfer.executable(),
                    transfer.directory(),
                    wire::Entry::Trial { run, snapshot },
                    transfer.launch_pins(),
                )?
            }
            Some(LaunchCustody::Broker(_)) | None => {
                return Err(ClaimError::invalid("launch_transfer_verified_custody_required"));
            }
        };
        let bytes = request.encode()?;
        state.issued = true;
        state.memory = Some(memory.clone());
        state.transfer = Some(Transfer {
            id,
            published: false,
            outcome: None,
            original: context.clone(),
            closed: false,
        });
        Ok(LaunchOffer { owner: self.clone(), id, bytes, _memory: memory })
    }
}

/// Snapshot actual borrowed handle identities under the source owner lock without reopening paths.
fn compose<'a>(
    id: [u8; 32],
    executable: &Path,
    directory: &Path,
    entry: wire::Entry,
    pins: impl Iterator<Item = &'a File>,
) -> Result<wire::Request, ClaimError> {
    let text = |path: &Path| {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| ClaimError::invalid("launch_transfer_path_encoding"))
    };
    let mut handles = Vec::new();
    for pin in pins {
        if handles.len() == wire::MAX_PINS {
            return Err(ClaimError::invalid("launch_transfer_pin_bound"));
        }
        handles.try_reserve(1).map_err(|cause| ClaimError::new("launch_transfer_pins", cause))?;
        handles.push(pin.as_raw_handle() as usize as u64);
    }
    Ok(wire::Request {
        version: 1,
        id,
        executable: text(executable)?,
        directory: text(directory)?,
        entry,
        pins: handles,
    })
}
