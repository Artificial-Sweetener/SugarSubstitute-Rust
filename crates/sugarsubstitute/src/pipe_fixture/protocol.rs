// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate wire metadata separately from actual transport authentication.
use super::Result;
use sugarsubstitute_contracts::lifecycle::{ConnectionId, RequestId, RunKey};
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    LifecyclePipeControl, LifecyclePipeOperationId,
};
use sugarsubstitute_supervision::lifecycle_protocol::{
    FrameCodec, WireHeader, WireMessage, WirePayload,
};

pub(super) fn header(
    run: RunKey,
    connection: Option<ConnectionId>,
    request: Option<RequestId>,
    ticks: i64,
) -> WireHeader {
    WireHeader { run, connection, request, deadline_qpc: ticks }
}
pub(super) fn validate(actual: WireHeader, expected: WireHeader) -> Result<()> {
    if actual != expected {
        return Err("Lifecycle frame metadata differs from its original bound request".into());
    }
    Ok(())
}
/// Encode into the transport's preclaimed loan; rejection retains and returns it.
pub(super) fn write(
    control: &LifecyclePipeControl,
    id: u64,
    message: WireMessage<'_>,
) -> Result<usize> {
    let mut buffer =
        control.take_write_buffer()?.ok_or("Lifecycle output buffer is still owned")?;
    let count = FrameCodec::encode(&message, buffer.as_mut_bytes())?;
    if let Err(rejected) = control.write(
        LifecyclePipeOperationId::new(id)?,
        control.original_deadline(),
        buffer,
        count,
    ) {
        // Passive buffer Drop returns the original storage to this same session.
        return Err(Box::new(rejected.error));
    }
    Ok(count)
}
pub(super) fn read(control: &LifecyclePipeControl, id: u64) -> Result<()> {
    control.read(LifecyclePipeOperationId::new(id)?, control.original_deadline())?;
    Ok(())
}
pub(super) fn message(header: WireHeader, payload: WirePayload<'_>) -> WireMessage<'_> {
    WireMessage { header, payload }
}
