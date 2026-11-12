// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project bounded kernel listener presence separately from exact established peer ownership.
use super::*;
#[allow(unsafe_code)]
mod native;
pub use native::TcpTableFailure;
/// Project complete native rows using the pinned ABI, independently of its estimated sizing output.
/// GetExtendedTcpTable's size may include padding; it never extends the fixed admitted buffer.
fn payload_words(words: &[u32], returned: usize) -> Result<usize, RegistryError> {
    use windows::Win32::NetworkManagement::IpHelper::{
        MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID,
    };
    let offset = std::mem::offset_of!(MIB_TCPTABLE_OWNER_PID, table);
    let stride = size_of::<MIB_TCPROW_OWNER_PID>();
    if offset != size_of::<u32>()
        || stride != 6 * size_of::<u32>()
        || returned > size_of_val(words)
        || returned < offset
        || !returned.is_multiple_of(std::mem::align_of::<MIB_TCPTABLE_OWNER_PID>())
    {
        return Err(RegistryError::invalid("listener_tcp_table_length"));
    }
    let count =
        words.first().copied().ok_or_else(|| RegistryError::invalid("listener_table_header"))?
            as usize;
    let payload = count
        .checked_mul(stride)
        .and_then(|bytes| bytes.checked_add(offset))
        .filter(|bytes| *bytes <= returned && *bytes <= size_of_val(words))
        .ok_or_else(|| RegistryError::invalid("listener_table_incomplete"))?;
    Ok(payload / size_of::<u32>())
}
pub(super) fn connected_owner(
    connection: ConnectedLoopback,
    storage: &mut TcpOwnerWorkspace,
    context: &JobContext,
) -> Result<u32, RegistryError> {
    if connection.client_port == 0 || connection.server_port == 0 {
        return Err(RegistryError::invalid("connected_loopback_ports"));
    }
    owner(
        native::read(
            storage,
            context,
            windows::Win32::NetworkManagement::IpHelper::TCP_TABLE_OWNER_PID_CONNECTIONS,
        )?,
        connection,
    )
}
pub(super) fn listening(
    port: u16,
    storage: &mut TcpOwnerWorkspace,
    context: &JobContext,
) -> Result<bool, RegistryError> {
    storage.require_lifecycle(context)?;
    if port == 0 {
        return Err(RegistryError::invalid("listener_port"));
    }
    presence(
        native::read(
            storage,
            context,
            windows::Win32::NetworkManagement::IpHelper::TCP_TABLE_OWNER_PID_LISTENER,
        )?,
        port,
    )
}
fn presence(words: &[u32], port: u16) -> Result<bool, RegistryError> {
    let count =
        words.first().copied().ok_or_else(|| RegistryError::invalid("listener_table_header"))?
            as usize;
    let expected = count
        .checked_mul(6)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| RegistryError::invalid("listener_table_count"))?;
    if words.len() != expected {
        return Err(RegistryError::invalid("listener_table_incomplete"));
    }
    let address = u32::from_ne_bytes([127, 0, 0, 1]);
    Ok(words[1..].chunks_exact(6).any(|row| {
        row[0] == 2
            && (row[1] == 0 || row[1] == address)
            && row[2] & 0xffff_0000 == 0
            && u16::from_be(row[2] as u16) == port
    }))
}
pub(super) fn check(context: &JobContext) -> Result<(), RegistryError> {
    use sugarsubstitute_contracts::execution::{JobClass, Operation};
    if context.spec().class != JobClass::InfrastructureLifecycle
        || !matches!(context.spec().operation, Operation::Readiness | Operation::NativeLifecycle)
        || context.cancellation_requested()
        || context.deadline().is_none_or(|deadline| context.now() >= deadline)
    {
        return Err(RegistryError::invalid("listener_context"));
    }
    Ok(())
}
/// Output transfers use ordinary blocking capacity and cannot borrow readiness admission.
pub(super) fn check_output(context: &JobContext) -> Result<(), RegistryError> {
    use sugarsubstitute_contracts::execution::{JobClass, Operation};
    if context.spec().class != JobClass::InfrastructureOrdinary
        || context.spec().operation != Operation::Infrastructure
        || context.cancellation_requested()
        || context.deadline().is_none_or(|deadline| context.now() >= deadline)
    {
        return Err(RegistryError::invalid("listener_output_context"));
    }
    Ok(())
}
/// Retire a read-only attached peer on its output worker or a separate lifecycle worker.
pub(super) fn check_retirement(context: &JobContext) -> Result<(), RegistryError> {
    if check(context).is_err() && check_output(context).is_err() {
        return Err(RegistryError::invalid("listener_retirement_context"));
    }
    Ok(())
}
fn owner(words: &[u32], connection: ConnectedLoopback) -> Result<u32, RegistryError> {
    let count =
        words.first().copied().ok_or_else(|| RegistryError::invalid("listener_table_header"))?
            as usize;
    let expected = count
        .checked_mul(6)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| RegistryError::invalid("listener_table_count"))?;
    if words.len() != expected {
        return Err(RegistryError::invalid("listener_table_incomplete"));
    }
    let address = u32::from_ne_bytes([127, 0, 0, 1]);
    let port = |value: u32| -> Option<u16> {
        (value & 0xffff_0000 == 0).then(|| u16::from_be(value as u16))
    };
    let mut pid = None;
    for row in words[1..].chunks_exact(6) {
        // MIB_TCP_STATE_ESTAB is 5; a LISTEN row cannot satisfy this connected proof.
        if row[0] == 5
            && row[1] == address
            && row[3] == address
            && port(row[2]) == Some(connection.server_port)
            && port(row[4]) == Some(connection.client_port)
            && (row[5] == 0 || pid.replace(row[5]).is_some())
        {
            return Err(RegistryError::invalid("listener_table_ambiguous"));
        }
    }
    pid.ok_or_else(|| RegistryError::invalid("listener_connected_tuple_absent"))
}
#[cfg(test)]
mod tests;
