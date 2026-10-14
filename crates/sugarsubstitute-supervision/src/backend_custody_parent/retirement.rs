// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Correlate checked resource and witness retirement under their original cutoffs.
use super::*;
use sugarsubstitute_contracts::lifecycle::ResourceKey;
#[derive(Clone, Copy)]
pub(super) struct Registration {
    pub(super) resource: ResourceKey,
    pub(super) stop: Option<(Moment, Option<u64>)>,
    pub(super) released: Option<ReceiptFacts>,
}
impl BackendCustodyParent {
    /// Request actual same-generation release after the parent writer is closed.
    /// Returns true only for no adopted registration or its correlated Released receipt.
    /// # Errors
    /// Preserves original admission/protocol causes and rejects foreign or renewed cleanup.
    pub fn advance_resource_release(
        &self,
        execution: &sugarsubstitute_execution::ExecutionClient,
        resource: ResourceKey,
        until: Moment,
    ) -> Result<bool, BackendCustodyParentFailure> {
        if resource.owner != self.key().owner() || !self.matches_execution(execution) {
            return Err(self.failure(reject("custody_release_owner")));
        }
        self.consume(
            NonZeroUsize::new(32).ok_or_else(|| self.failure(reject("custody_release_bound")))?,
        )?;
        let counter = execution.encode_lifecycle_deadline(until).map_err(|cause| {
            self.failure(NativeObservationError::new("custody_release_counter", cause))
        })?;
        self.storage.check().map_err(|cause| self.failure(cause))?;
        Ok(self
            .try_access(|state| {
                let Some(at) = state
                    .registrations
                    .iter()
                    .position(|slot| slot.is_some_and(|entry| entry.resource == resource))
                else {
                    return Ok(!state.pending.is_some_and(|(frame, _)| {
                        matches!(frame.command,
                    BackendCustodyCommand::Prepare { resource: actual, .. } if actual == resource)
                    }));
                };
                let entry = state.registrations[at]
                    .as_mut()
                    .ok_or_else(|| reject("custody_release_registration"))?;
                if entry.released.is_some() {
                    return Ok(true);
                }
                if entry.stop.is_some_and(|(original, _)| original != until) {
                    return Err(reject("custody_release_renewed"));
                }
                let stop = entry.stop.get_or_insert((until, None));
                if let Some(facts) = state.receipt {
                    if facts.receipt == BackendCustodyReceipt::Released(resource)
                        && stop.1 == Some(facts.nonce)
                    {
                        state.receipt = None;
                        entry.released = Some(facts);
                        return Ok(true);
                    }
                    if matches!(facts.receipt, BackendCustodyReceipt::Prepared(actual)
                    | BackendCustodyReceipt::Root(actual) if actual == resource)
                    {
                        // The original creator has finished. Its unacknowledged remote
                        // outputs remain retained until actual helper exit.
                        state.receipt = None;
                    }
                }
                if stop.1.is_none() && state.pending.is_none() && state.receipt.is_none() {
                    let nonce =
                        state.nonce.checked_add(1).ok_or_else(|| reject("custody_parent_nonce"))?;
                    state.nonce = nonce;
                    state.pending = Some((
                        BackendCustodyFrame {
                            key: self.key(),
                            nonce,
                            command: BackendCustodyCommand::Stop {
                                resource,
                                until_ticks: counter.ticks(),
                                frequency: counter.frequency(),
                            },
                        },
                        until,
                    ));
                    stop.1 = Some(nonce);
                    state.advance()?;
                }
                Ok(false)
            })?
            .unwrap_or(false))
    }
    /// Borrow only the real sealed release for the exact retained resource generation.
    pub(crate) fn release_acknowledgement(
        &self,
        resource: ResourceKey,
    ) -> Option<BackendCustodyAcknowledgement> {
        self.state
            .as_ref()
            .and_then(|owner| {
                lock(owner).as_ref().and_then(|state| {
                    state
                        .registrations
                        .iter()
                        .flatten()
                        .find(|entry| entry.resource == resource)
                        .and_then(|entry| entry.released)
                })
            })
            .map(|facts| BackendCustodyAcknowledgement { facts, parent: self.clone() })
    }
    /// Finish the one original witness after all managed writers are closed.
    /// Returns true only after the same nonce's actual Finished receipt.
    /// # Errors
    /// Preserves actual transport errors and rejects changed original cleanup deadlines.
    pub fn advance_finish(
        &self,
        execution: &sugarsubstitute_execution::ExecutionClient,
        until: Moment,
    ) -> Result<bool, BackendCustodyParentFailure> {
        if !self.matches_execution(execution) {
            return Err(self.failure(reject("custody_finish_owner")));
        }
        self.consume(
            NonZeroUsize::new(32).ok_or_else(|| self.failure(reject("custody_finish_bound")))?,
        )?;
        Ok(self
            .try_access(|state| {
                let finish = state.finish.get_or_insert_with(|| finish::Finish::new(until));
                let progress = finish.advance(
                    self.key(),
                    until,
                    finish::Exchange {
                        next_nonce: &mut state.nonce,
                        pending: &mut state.pending,
                        receipt: &mut state.receipt,
                        io_pending: state.write.is_some() || state.read.is_some(),
                    },
                    || {
                        self.storage.check()?;
                        execution.encode_lifecycle_deadline(until).map_err(|cause| {
                            NativeObservationError::new("custody_finish_counter", cause)
                        })
                    },
                )?;
                if progress == finish::Progress::Queued {
                    state.advance()?;
                }
                Ok(progress == finish::Progress::Finished)
            })?
            .unwrap_or(false))
    }
}
