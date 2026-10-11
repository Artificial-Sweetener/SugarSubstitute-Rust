// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Construct the sole allowed executable role without arbitrary command or environment input.
use super::*;
use crate::{WindowsLaunchBuffers, WindowsLaunchCommand};
use std::{ffi::OsString, os::windows::io::AsRawHandle, path::PathBuf};
use sugarsubstitute_execution::windows_fixture::{FixtureDeadline, FixtureForwardingEvents};

/// Exact current-executable held role, original deadline and one execution-owned pin.
/// Construction grants no containment authority or admission and performs no process effect.
pub struct FixtureDescendantInput {
    pub(super) command: WindowsLaunchBuffers,
    pub(super) module: PathBuf,
    pub(super) events: Option<FixtureForwardingEvents>,
    pub(super) correlation: NativeCorrelation,
    pub(super) deadline: FixtureDeadline,
    pub(super) required: u64,
}
impl FixtureDescendantInput {
    /// Select only this executable's held-child role with exact four handles and empty environment.
    /// Exit code must be0/37/259. The native worker verifies current executable identity again.
    pub fn current(
        events: FixtureForwardingEvents,
        deadline: FixtureDeadline,
        correlation: NativeCorrelation,
        exit_code: u32,
    ) -> Result<Self, RegistryError> {
        if !matches!(exit_code, 0 | 37 | 259) {
            return Err(RegistryError::invalid("fixture_descendant_exit_code"));
        }
        let module = std::env::current_exe()
            .map_err(|cause| RegistryError::new("fixture_descendant_module", cause))?;
        let cwd =
            module.parent().ok_or_else(|| RegistryError::invalid("fixture_descendant_cwd"))?;
        let argv = {
            let view = events.borrow_inheritance();
            let raw = [
                view.ready.as_raw_handle() as usize,
                view.release.as_raw_handle() as usize,
                view.challenge.as_raw_handle() as usize,
                view.response.as_raw_handle() as usize,
            ];
            arguments(raw, deadline, correlation, exit_code)
        };
        let command = WindowsLaunchCommand::from_os(&module, cwd, &argv)
            .map_err(|cause| RegistryError::new("fixture_descendant_command", cause))?
            .into_native_parts();
        let bytes = command
            .application_name
            .capacity()
            .checked_add(command.command_line.capacity())
            .and_then(|n| n.checked_add(command.working_directory.capacity()))
            .and_then(|n| n.checked_mul(2))
            .ok_or_else(|| RegistryError::invalid("fixture_descendant_capacity"))?;
        let module_bytes = module
            .capacity()
            .checked_mul(2)
            .ok_or_else(|| RegistryError::invalid("fixture_descendant_capacity"))?;
        let required = u64::try_from(
            bytes
                .checked_add(module_bytes)
                .and_then(|n| n.checked_add(4096))
                .ok_or_else(|| RegistryError::invalid("fixture_descendant_capacity"))?,
        )
        .map_err(|cause| RegistryError::new("fixture_descendant_capacity", cause))?;
        Ok(Self { command, module, events: Some(events), correlation, deadline, required })
    }
    /// Required admitted byte reservation for actual fixed state and retained native input capacities.
    pub const fn required_reservation_bytes(&self) -> u64 {
        self.required
    }
}
pub(super) fn arguments(
    raw: [usize; 4],
    deadline: FixtureDeadline,
    correlation: NativeCorrelation,
    exit_code: u32,
) -> Vec<OsString> {
    let mut argv = vec![OsString::from("--internal-native-held-child")];
    for (flag, value) in [
        ("--ready", raw[0] as u64),
        ("--release", raw[1] as u64),
        ("--challenge", raw[2] as u64),
        ("--response", raw[3] as u64),
        ("--deadline-qpc", deadline.ticks()),
        ("--qpc-frequency", deadline.frequency()),
        ("--correlation", correlation.get()),
        ("--exit-code", u64::from(exit_code)),
    ] {
        argv.push(flag.into());
        argv.push(value.to_string().into());
    }
    argv
}
