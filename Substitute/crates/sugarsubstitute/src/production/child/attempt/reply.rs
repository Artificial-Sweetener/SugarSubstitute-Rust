// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify an authenticated exact attempt reply without manufacturing settlement.
use super::{AcquisitionFailure, Result};
use sugarsubstitute_contracts::lifecycle::LifecycleFailureCode;
use sugarsubstitute_supervision::lifecycle_protocol::{
    DecodedFrame, DecodedPayload, ReplyOutcome, WireHeader,
};

pub(super) enum SettlementReply {
    Acknowledged,
    Failed(AcquisitionFailure),
}
/// This is data validation only; the caller owns actual frame provenance and consumption.
pub(super) fn classify(
    expected: WireHeader,
    received: &DecodedFrame<'_>,
) -> Result<SettlementReply> {
    if received.header != expected {
        return Err("Attempt reply differs from original request".into());
    }
    match received.payload {
        DecodedPayload::Reply(ReplyOutcome::Acknowledged) => Ok(SettlementReply::Acknowledged),
        DecodedPayload::Reply(ReplyOutcome::Failed(LifecycleFailureCode::Cleanup)) => {
            Ok(SettlementReply::Failed(AcquisitionFailure::Failed(LifecycleFailureCode::Cleanup)))
        }
        _ => Err("Attempt settlement was not acknowledged".into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_contracts::lifecycle::{
        ConnectionId, CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RequestId,
        RunGeneration, RunKey,
    };
    use sugarsubstitute_supervision::lifecycle_protocol::{FrameCodec, WireMessage, WirePayload};
    #[test]
    fn actual_codec_cleanup_failure_is_not_acknowledgement_and_foreign_reply_cannot_settle()
    -> Result<()> {
        let header = WireHeader {
            run: RunKey {
                owner: CoordinatorKey {
                    namespace: NamespaceId::new([1; 32]),
                    epoch: CoordinatorEpoch::new([1; 16])?,
                },
                instance: InstanceId::new(1)?,
                generation: RunGeneration::new(1)?,
            },
            connection: Some(ConnectionId::new(1)?),
            request: Some(RequestId::new(2)?),
            deadline_qpc: 100,
        };
        let mut bytes = [0; 256];
        let count = FrameCodec::encode(
            &WireMessage {
                header,
                payload: WirePayload::Reply(ReplyOutcome::Failed(LifecycleFailureCode::Cleanup)),
            },
            &mut bytes,
        )?;
        let decoded = FrameCodec::decode(&bytes[..count])?;
        assert!(matches!(
            classify(header, &decoded)?,
            SettlementReply::Failed(AcquisitionFailure::Failed(LifecycleFailureCode::Cleanup))
        ));
        let foreign = WireHeader { request: Some(RequestId::new(3)?), ..header };
        assert!(classify(foreign, &decoded).is_err());
        let count = FrameCodec::encode(
            &WireMessage { header, payload: WirePayload::Reply(ReplyOutcome::Acknowledged) },
            &mut bytes,
        )?;
        let decoded = FrameCodec::decode(&bytes[..count])?;
        assert!(matches!(classify(header, &decoded)?, SettlementReply::Acknowledged));
        Ok(())
    }
}
