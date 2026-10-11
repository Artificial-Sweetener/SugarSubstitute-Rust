// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Public pure input/state contracts without native child creation.
#![cfg(windows)]
use std::error::Error;
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, CoordinatorKey, NamespaceId, ProcessId, ProcessKey,
};
use sugarsubstitute_supervision::{
    LaunchEnvironment, RegistryAdmissionError, WindowsLaunchCommand, WindowsLaunchInput,
    WindowsProcessRegistry,
};
type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn public_launch_input_charges_native_buffers_and_independent_family_storage() -> TestResult {
    let executable: Vec<_> = "E:\\fixture\\sugarsubstitute.exe".encode_utf16().collect();
    let cwd: Vec<_> = "E:\\fixture".encode_utf16().collect();
    let command = WindowsLaunchCommand::from_utf16(
        &executable,
        &cwd,
        &[Vec::new(), "雪".encode_utf16().collect()],
    )?;
    let buffers = command.into_native_parts();
    let expected = ((buffers.application_name.capacity()
        + buffers.working_directory.capacity()
        + buffers.command_line.capacity())
        * 2) as u64;
    let environment = LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?;
    let environment_bytes = environment.allocated_bytes()?;
    let command = WindowsLaunchCommand::from_utf16(
        &executable,
        &cwd,
        &[Vec::new(), "雪".encode_utf16().collect()],
    )?;
    let input = WindowsLaunchInput::new(command, environment)?;
    assert!(input.required_reservation_bytes() > expected + environment_bytes);
    assert!(!format!("{input:?}").contains("fixture"));
    Ok(())
}

#[test]
fn public_registry_matches_exact_coordinator_without_pid_authority() -> TestResult {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([1; 16])?,
    };
    let mut registry = WindowsProcessRegistry::new(owner)?;
    assert!(registry.is_empty());
    let key = ProcessKey { owner, id: ProcessId::new(1)? };
    assert!(matches!(registry.snapshot(key), Err(RegistryAdmissionError::Stale)));
    let mut foreign = key;
    foreign.owner.epoch = CoordinatorEpoch::new([2; 16])?;
    assert!(matches!(registry.snapshot(foreign), Err(RegistryAdmissionError::ForeignOwner)));
    registry.close_launch_admission();
    assert_eq!(registry.next_cleanup(), None);
    assert!(registry.is_empty());
    Ok(())
}
