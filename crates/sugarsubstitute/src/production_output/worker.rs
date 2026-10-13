// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode one actual charged frame and parse at most32KiB on the finite worker.
use super::*;
use sugarsubstitute_contracts::{
    execution::JobContext,
    transcript::{OutputBatch, TranscriptChange},
};
use sugarsubstitute_supervision::lifecycle_protocol::{DecodedPayload, ReplyOutcome};
pub(super) struct Parsed {
    pub(super) delivery: Option<std::num::NonZeroU64>,
    pub(super) change: Option<TranscriptChange>,
    pub(super) snapshot: Option<OutputSnapshot>,
}
pub(super) fn check(context: &JobContext) -> Result<(), OutputError> {
    if context.cancellation_requested() {
        return Err(OutputError::Cancelled);
    }
    if context.deadline().is_none_or(|deadline| context.now() >= deadline) {
        return Err(OutputError::Deadline);
    }
    Ok(())
}
pub(super) fn parse(
    source: &state::Source,
    binding: OutputBinding,
    pending: Pending,
    ack: Option<std::num::NonZeroU64>,
    context: &JobContext,
) -> Result<Parsed, OutputError> {
    source.check(context)?;
    let _scratch = context.memory().claim(TRANSFER_BYTES).map_err(OutputError::Claim)?;
    let mut state = source.state.lock().map_err(|_| OutputError::Poisoned)?;
    let state::State { transcript, frame } = &mut *state;
    let frame = frame.as_ref().ok_or(OutputError::Order)?;
    let decoded = FrameCodec::decode(frame.as_bytes())?;
    if decoded.header != pending.header {
        return Err(OutputError::Identity);
    }
    let DecodedPayload::Reply(reply) = decoded.payload else { return Err(OutputError::Identity) };
    match reply {
        ReplyOutcome::OutputIdle(lease) if lease == binding.lease => {
            Ok(Parsed { delivery: None, change: None, snapshot: None })
        }
        ReplyOutcome::OutputDelivery(delivery) => {
            if delivery.lease != binding.lease
                || delivery.delivery_id.get()
                    != ack
                        .map_or(Some(1), |v| v.get().checked_add(1))
                        .ok_or(OutputError::Identity)?
            {
                return Err(OutputError::Identity);
            }
            let parsed = apply_delivery(transcript, binding, delivery, &source.custody)?;
            drop(state);
            check(context)?;
            Ok(parsed)
        }
        _ => Err(OutputError::Identity),
    }
}

/// Apply already-validated matching data; this creates no frame or native proof.
pub(super) fn apply_delivery(
    transcript: &mut sugarsubstitute_application::transcript::TranscriptStore,
    binding: OutputBinding,
    delivery: sugarsubstitute_supervision::lifecycle_protocol::OutputDelivery<'_>,
    custody: &snapshot::SourceCustody,
) -> Result<Parsed, OutputError> {
    let batch = OutputBatch::new(
        binding.resource,
        delivery.stream,
        delivery.sequence,
        delivery.bytes,
        delivery.gap,
        delivery.end,
    )?;
    let change = transcript.apply(&batch)?;
    drop(batch);
    let view =
        transcript.snapshot_tail(sugarsubstitute_application::transcript::MAX_SNAPSHOT_RECORDS)?;
    Ok(Parsed {
        delivery: Some(delivery.delivery_id),
        change: Some(change),
        snapshot: Some(OutputSnapshot::new(view, custody.clone())),
    })
}
