// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify inert adapter admission without reading the user's environment.

use super::WindowsEnvironment;
use crate::entry_preparation::{
    cause::EntryEnvironmentCause,
    port::{AllocationState, EnvironmentPort},
};

#[test]
fn unacquired_native_adapter_rejects_read_and_free_without_effects() {
    let mut owner = WindowsEnvironment::new();
    assert_eq!(owner.allocation_state(), AllocationState::Vacant);
    assert!(matches!(owner.read_unit(0), Err(EntryEnvironmentCause::InvalidNativeState)));
    assert!(matches!(owner.free(), Err(EntryEnvironmentCause::InvalidNativeState)));
    assert_eq!(owner.allocation_state(), AllocationState::Vacant);
}
