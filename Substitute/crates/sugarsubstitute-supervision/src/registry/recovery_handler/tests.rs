// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify the physical terminal matching policy without manufacturing process/watch authority.
use super::*;
use sugarsubstitute_contracts::lifecycle::*;
#[test]
fn independent_recovery_handler_exit_requires_exact_family_correlation_pid_birth_and_preserves259()
-> Result<(), Box<dyn std::error::Error>> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let process = ProcessKey { owner, id: ProcessId::new(3)? };
    let actual = RecoveryProcessIdentity::new(4, 5)?;
    let exit = ProcessExit {
        correlation: sugarsubstitute_execution::windows::NativeCorrelation::new(3)?,
        process_id: 4,
        creation_time_100ns: 5,
        exit_code: 259,
    };
    validate_recovery_exit(exit, process, actual)?;
    for changed in [
        ProcessExit { process_id: 6, ..exit },
        ProcessExit { creation_time_100ns: 6, ..exit },
        ProcessExit {
            correlation: sugarsubstitute_execution::windows::NativeCorrelation::new(6)?,
            ..exit
        },
    ] {
        assert!(validate_recovery_exit(changed, process, actual).is_err());
    }
    assert_eq!(exit.exit_code, 259);
    Ok(())
}
