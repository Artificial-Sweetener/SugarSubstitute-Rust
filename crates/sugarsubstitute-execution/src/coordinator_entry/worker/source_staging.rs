// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt each encoder outcome before observing original preparation controls.

use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};
use sugarsubstitute_contracts::execution::MemoryClaim;

pub(super) enum EntrySourceOutcome {
    Launch(Result<usize, EntryTransportError>),
    Hello(Result<usize, EntryTransportError>),
    Transfer(Result<(), EntryTransportError>),
}

impl EntrySourceOutcome {
    fn result(self) -> Result<usize, EntryTransportError> {
        match self {
            Self::Launch(result) | Self::Hello(result) => result,
            Self::Transfer(result) => result.map(|()| 0),
        }
    }
}

impl Active {
    pub(super) fn stage_source(
        &mut self,
        core: &Core,
        input_claim: MemoryClaim,
    ) -> Result<(), EntryTransportError> {
        self.source_input_claim = Some(input_claim);
        self.source_owner =
            self.work.input.as_ref().and_then(|input| input.request.original_owner()).cloned();
        self.source_fence(core, true)?;
        let input = self.work.input.as_ref().ok_or(EntryTransportError::InvalidPhase)?;
        let request = EntryRequestId::new(input.source_setup.ticks() as u64)
            .map_err(EntryTransportError::Shape)?;
        let header = protocol::Header {
            namespace: input.facts.namespace(),
            epoch: input.candidate,
            request,
            source: input.source_setup,
            nonce: [0; 32],
        };
        self.client =
            Some(Box::new(ClientStaging { header, length: 0, hello: [0; protocol::HEADER_BYTES] }));
        // Active owns this exact returned outcome before any lazy original check.
        self.source_pending = Some(EntrySourceOutcome::Launch(protocol::stage_launch(
            &mut self.connections[0].write.as_mut().ok_or(EntryTransportError::InvalidPhase)?.bytes,
            input.request.request(),
        )));
        let length = self.complete_source_step(core)?;
        self.client.as_mut().ok_or(EntryTransportError::InvalidPhase)?.length = length;

        self.source_fence(core, true)?;
        let client = self.client.as_mut().ok_or(EntryTransportError::InvalidPhase)?;
        self.source_pending = Some(EntrySourceOutcome::Hello(protocol::encode(
            &mut client.hello,
            &header,
            protocol::Payload::Hello,
        )));
        self.complete_source_step(core)?;

        // Both original and current client fences precede source destruction.
        self.source_fence(core, true)?;
        let dropped = catch_unwind(AssertUnwindSafe(|| drop(self.work.input.take())));
        self.source_pending = Some(EntrySourceOutcome::Transfer(dropped.map_err(|payload| {
            let cause = crate::task::panic_cause(payload);
            let mut native = NativeObservationError::new(
                "entry_source_destruction_panic",
                std::io::Error::other("Source destruction unwound"),
            );
            native.retain_panic(&cause);
            EntryTransportError::native(native)
        })));
        self.complete_source_step(core)?;
        // The outside original alias keeps control and custody alive throughout
        // destruction and its post-observation. Successful charged staging ends
        // that source obligation; later original cancellation has no source data.
        drop(self.source_owner.take());
        drop(self.source_input_claim.take());
        Ok(())
    }

    fn source_fence(&self, core: &Core, publication: bool) -> Result<(), EntryTransportError> {
        // Keep the destination result owned before the final original provider.
        let sampled = core.clock.metadata().map_err(|cause| {
            EntryTransportError::Clock(NativeObservationError::new(
                "entry_source_staging_clock",
                cause,
            ))
        });
        let original = if let Some(owner) = &self.source_owner {
            owner
                .checked(|| {
                    let binding = owner.original_binding()?;
                    if publication && sampled.is_ok() {
                        binding.check_publication(core, binding.setup())
                    } else {
                        binding.check(core, binding.setup())
                    }
                })
                .map(Some)
                .map_err(|cause| {
                    EntryTransportError::Preparation(
                        crate::entry_preparation::EntryPreparationError::new(cause, owner.clone()),
                    )
                })
        } else {
            Ok(None)
        };
        let now = match (sampled, original) {
            (Err(mut first), Err(after)) => {
                first.retain_cleanup(after);
                return Err(first);
            }
            (Err(first), Ok(_)) => return Err(first),
            (Ok(_), Err(after)) => return Err(after),
            (Ok(now), Ok(original)) => original.map_or(now, |original| now.max(original)),
        };
        // No provider runs between this genuine record/lease/ancestry check and
        // the next effect. Caller-supplied cancellation ports grant no liveness.
        core.validate_preparation_client(
            &lock(&core.state),
            &self.context,
            now,
            self.work.shared.original_preparation,
        )
        .map_err(|cause| {
            EntryTransportError::native(NativeObservationError::new(
                "entry_current_source_staging",
                cause,
            ))
        })
    }

    fn complete_source_step(&mut self, core: &Core) -> Result<usize, EntryTransportError> {
        let post = self.source_fence(core, false);
        let result = self.source_pending.take().ok_or(EntryTransportError::InvalidPhase)?.result();
        let observed = match (result, post) {
            (Err(mut first), Err(after)) => {
                first.retain_cleanup(after);
                Err(first)
            }
            (Err(first), Ok(())) => Err(first),
            (Ok(_), Err(after)) => Err(after),
            (Ok(length), Ok(())) => Ok(length),
        };
        if observed.is_err()
            && let Some(owner) = &self.source_owner
        {
            owner.close_publication();
        }
        observed
    }

    pub(super) fn preserve_pending_source_failure(&mut self) {
        if let Some(pending) = self.source_pending.take()
            && let Err(first) = pending.result()
        {
            self.outcome = Some(JobOutcome::Failure(first));
        }
    }
}

#[cfg(test)]
mod tests;
