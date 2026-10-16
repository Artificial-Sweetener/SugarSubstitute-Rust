// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve full producing peaks and exact immutable native-name capacity through final aliases.
use super::*;
use crate::protected_storage::native_name::NativeName;
use std::path::{Path, PathBuf};

#[test]
fn native_name_retains_actual_immutable_capacity_and_original_claim_through_last_alias()
-> TestResult {
    admitted(|context, _| {
        let (mut storage, operation) = setup(&context, Filesystem::new())?;
        let owner = storage.shared._allocation.clone();
        let original = owner.native_grant();
        let initial = owner.fixture_residual_bytes().ok_or("Original ledger absent")?;
        let data_bytes = NativeName::diagnostic_owned_layout()?.retained_bytes();
        let control_bytes = NativeName::diagnostic_control_layout()?.retained_bytes();
        let mut retained_bytes = 0;
        let mut aliases = Vec::new();
        for capacity in [512, 192 * 1024] {
            let allocation = original.owned_name(&context)?;
            let control = original.namespace_control()?;
            assert_eq!(
                owner.fixture_residual_bytes(),
                Some(initial + retained_bytes + data_bytes + control_bytes)
            );
            let mut path = PathBuf::new();
            path.try_reserve_exact(capacity)?;
            path.push(r"E:\fixture-retained-native-name");
            let actual = path.capacity();
            assert_eq!(actual, capacity);
            let mut input = Some((path, allocation, control, original.clone()));
            let name = NativeName::retained(&mut input)?;
            assert!(input.is_none());
            assert_eq!(name.capacity()?, actual);
            retained_bytes += actual + control_bytes;
            assert_eq!(owner.fixture_residual_bytes(), Some(initial + retained_bytes));
            aliases.push(name.clone());
            drop(name);
            assert_eq!(owner.fixture_residual_bytes(), Some(initial + retained_bytes));
        }
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        drop(original);
        drop(owner);
        assert!(context.memory().claim(1).is_err());
        for alias in &aliases {
            assert_eq!(alias.path()?, Path::new(r"E:\fixture-retained-native-name"));
        }
        drop(aliases.pop().ok_or("Maximum-capacity final alias absent")?);
        assert!(context.memory().claim(1).is_err());
        drop(aliases);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}
