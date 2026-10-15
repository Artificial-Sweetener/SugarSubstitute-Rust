// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Supply external failures to an actual execution-admitted parent without peer authority.
use super::*;
use sugarsubstitute_contracts::execution::{JobSpec, Scope};
use sugarsubstitute_execution::ExecutionClient;

/// Construct through the genuine pipe admission and original funded parent constructor.
/// This fixture authenticates no peer and grants no native acknowledgement authority.
pub(crate) fn parent(
    client: &ExecutionClient,
    scope: &Scope,
    spec: JobSpec,
    key: BackendCustodyKey,
    until: Moment,
) -> Result<BackendCustodyParent, Box<dyn std::error::Error>> {
    let mut session = Some(client.reserve_backend_custody_pipe_server(scope, spec, key, until)?);
    Ok(BackendCustodyParent::new(&mut session)?)
}

/// Preserve an external transport failure through the same production first-cause owner.
pub(crate) fn fail(
    parent: &BackendCustodyParent,
    cause: NativeObservationError,
) -> Result<BackendCustodyParentFailure, Box<dyn std::error::Error>> {
    parent.access::<()>(|_| Err(cause)).err().ok_or_else(|| "Original failure lost".into())
}

/// Feed external wire facts through the same read/write correlation owner as production.
/// This exercises cached response observation, not peer authentication or native issue.
pub(crate) fn reply(
    parent: &BackendCustodyParent,
    nonce: u64,
    command: BackendCustodyCommand,
    receipt: BackendCustodyReceipt,
) -> Result<(), BackendCustodyParentFailure> {
    parent.access(|state| {
        let key = parent.key();
        let original = BackendCustodyFrame { key, nonce, command };
        let operation = LifecyclePipeOperationId::new(8)
            .map_err(|cause| NativeObservationError::new("fixture_read", cause))?;
        let write = LifecyclePipeOperationId::new(7)
            .map_err(|cause| NativeObservationError::new("fixture_write", cause))?;
        state::correlate_written(Some(write), write, BACKEND_CUSTODY_FRAME_BYTES as u32)?;
        let encoded =
            BackendCustodyFrame { key, nonce, command: BackendCustodyCommand::Receipt(receipt) }
                .encode()
                .map_err(|cause| NativeObservationError::new("fixture_frame", cause))?;
        let frame = state::decode_received(Some(operation), operation, key, &encoded)?;
        state.receipt = Some(state::correlate_reply(Some(original), frame)?);
        Ok(())
    })
}
