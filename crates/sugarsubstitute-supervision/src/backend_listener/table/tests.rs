// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise counted table projections without inferring peer authority from listener presence.
use super::*;
#[path = "tests/native.rs"]
mod sdk;
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
#[test]
fn native_estimated_padding_projects_only_complete_counted_rows() -> Result<(), RegistryError> {
    let mut words = row(5, 42, 12345);
    words.extend([0xdeadbeef, 0xabad1dea]);
    let count = payload_words(&words, size_of_val(words.as_slice()))?;
    assert_eq!(count, 7);
    assert_eq!(owner(&words[..count], ConnectedLoopback::new(12345, 8188)?)?, 42);
    assert!(owner(&words, ConnectedLoopback::new(12345, 8188)?).is_err());
    assert_eq!(payload_words(&[0, 0, 0], 12)?, 1);
    Ok(())
}
#[test]
fn native_projection_rejects_truncated_overbound_and_count_overflow() {
    let words = row(5, 42, 12345);
    assert!(payload_words(&words, 24).is_err());
    assert!(payload_words(&words, 29).is_err());
    assert!(payload_words(&[1, 2, 3, 4, 5, 6, 7, 0, 0], 29).is_err());
    assert!(payload_words(&words, 0).is_err());
    assert!(payload_words(&[u32::MAX, 0, 0], 12).is_err());
    assert!(payload_words(&[], 0).is_err());
}
#[test]
fn listener_presence_is_only_exact_ipv4_or_wildcard_port_and_never_connected_proof()
-> Result<(), RegistryError> {
    let mut words = row(2, 42, 0);
    assert!(presence(&words, 8188)?);
    assert!(!presence(&words, 8189)?);
    assert!(owner(&words, ConnectedLoopback::new(12345, 8188)?).is_err());
    words[2] = 0;
    assert!(presence(&words, 8188)?);
    words[2] = u32::from_ne_bytes([192, 168, 1, 2]);
    assert!(!presence(&words, 8188)?);
    words[2] = u32::from_ne_bytes([127, 0, 0, 1]);
    words[3] |= 0x1_0000;
    assert!(!presence(&words, 8188)?);
    assert!(!presence(&row(5, 42, 12345), 8188)?);
    assert!(!presence(&[0], 8188)?);
    assert!(presence(&[2, 0], 8188).is_err());
    Ok(())
}

#[test]
fn complete_busy_host_table_beyond_old_64k_keeps_exact_established_tuple()
-> Result<(), RegistryError> {
    let count = 20_000;
    let mut words = vec![0; count * 6 + 3];
    words[0] = count as u32;
    let target = row(5, 42, 12345);
    let offset = 1 + (count - 1) * 6;
    words[offset..offset + 6].copy_from_slice(&target[1..]);
    let bytes = size_of_val(words.as_slice());
    assert!(bytes > 64 * 1024 && bytes < TCP_OWNER_WORKSPACE_BYTES as usize);
    let projected = payload_words(&words, bytes)?;
    assert_eq!(owner(&words[..projected], ConnectedLoopback::new(12345, 8188)?)?, 42);
    assert!(payload_words(&words[..16 * 1024], bytes).is_err());
    Ok(())
}
#[test]
fn native_oversize_retains_required_bytes_raw122_and_secondary_control()
-> Result<(), Box<dyn std::error::Error>> {
    use std::error::Error;
    let error = native::complete(
        122,
        TCP_OWNER_WORKSPACE_BYTES as u32,
        TCP_OWNER_WORKSPACE_BYTES as u32 + 4096,
        Err(RegistryError::invalid("listener_context")),
    )
    .err()
    .ok_or("Expected overflow")?;
    let cause = error
        .source()
        .and_then(|cause| cause.downcast_ref::<TcpTableFailure>())
        .ok_or("Missing native size")?;
    assert_eq!(cause.supplied_bytes(), TCP_OWNER_WORKSPACE_BYTES as u32);
    assert_eq!(cause.required_bytes(), TCP_OWNER_WORKSPACE_BYTES as u32 + 4096);
    assert_eq!(
        cause
            .source()
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .and_then(std::io::Error::raw_os_error),
        Some(122)
    );
    assert_eq!(error.cleanup_causes().len(), 1);
    assert_eq!(error.cleanup_causes()[0].stage(), "listener_context");
    assert!(native::complete(0, 64, 64, Err(RegistryError::invalid("listener_context"))).is_err());
    Ok(())
}
