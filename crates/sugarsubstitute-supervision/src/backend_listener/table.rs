// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read one bounded complete TCP table and reject ambiguous or non-established tuples.
use super::*;
#[allow(unsafe_code)]
mod native {
    use super::*;
    use windows::Win32::{
        Foundation::ERROR_SUCCESS,
        NetworkManagement::IpHelper::{GetExtendedTcpTable, TCP_TABLE_OWNER_PID_CONNECTIONS},
        Networking::WinSock::AF_INET,
    };
    pub(super) struct Table {
        pub(super) words: Vec<u32>,
        _claim: sugarsubstitute_contracts::execution::MemoryClaim,
    }
    pub(super) fn read(context: &JobContext) -> Result<Table, RegistryError> {
        check(context)?;
        let claim = context
            .memory()
            .claim(64 * 1024)
            .map_err(|e| RegistryError::new("listener_table_memory", e))?;
        let mut bytes = vec![0u32; 16 * 1024];
        let mut length = (bytes.len() * size_of::<u32>()) as u32;
        // SAFETY: Aligned initialized storage has the complete advertised 64KiB capacity.
        // Windows either fills a complete table or returns an error; no retry or partial table is accepted.
        let code = unsafe {
            GetExtendedTcpTable(
                Some(bytes.as_mut_ptr().cast()),
                &mut length,
                false,
                u32::from(AF_INET.0),
                TCP_TABLE_OWNER_PID_CONNECTIONS,
                0,
            )
        };
        check(context)?;
        if code != ERROR_SUCCESS.0 {
            return Err(RegistryError::new(
                "listener_tcp_table",
                std::io::Error::from_raw_os_error(code as i32),
            ));
        }
        if length as usize > bytes.len() * size_of::<u32>()
            || length < 4
            || !length.is_multiple_of(4)
        {
            return Err(RegistryError::invalid("listener_tcp_table_length"));
        }
        bytes.truncate(length as usize / 4);
        Ok(Table { words: bytes, _claim: claim })
    }
}
pub(super) fn connected_owner(
    connection: ConnectedLoopback,
    context: &JobContext,
) -> Result<u32, RegistryError> {
    if connection.client_port == 0 || connection.server_port == 0 {
        return Err(RegistryError::invalid("connected_loopback_ports"));
    }
    owner(&native::read(context)?.words, connection)
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
mod tests {
    use super::*;
    fn row(state: u32, pid: u32, client: u16) -> Vec<u32> {
        vec![
            1,
            state,
            u32::from_ne_bytes([127, 0, 0, 1]),
            u32::from(8188u16.to_be()),
            u32::from_ne_bytes([127, 0, 0, 1]),
            u32::from(client.to_be()),
            pid,
        ]
    }
    #[test]
    fn exact_connected_tuple_selects_server_owner() -> Result<(), RegistryError> {
        assert_eq!(owner(&row(5, 42, 12345), ConnectedLoopback::new(12345, 8188)?)?, 42);
        Ok(())
    }
    #[test]
    fn listen_and_other_connections_never_prove_readiness() -> Result<(), RegistryError> {
        let connection = ConnectedLoopback::new(12345, 8188)?;
        assert!(owner(&row(2, 42, 12345), connection).is_err());
        assert!(owner(&row(5, 42, 12346), connection).is_err());
        assert!(owner(&row(5, 0, 12345), connection).is_err());
        Ok(())
    }
    #[test]
    fn duplicate_and_truncated_tables_are_rejected() -> Result<(), RegistryError> {
        let mut words = row(5, 42, 12345);
        words[0] = 2;
        assert!(owner(&words, ConnectedLoopback::new(12345, 8188)?).is_err());
        let repeated = words[1..].to_vec();
        words.extend(repeated);
        assert!(owner(&words, ConnectedLoopback::new(12345, 8188)?).is_err());
        Ok(())
    }
}
