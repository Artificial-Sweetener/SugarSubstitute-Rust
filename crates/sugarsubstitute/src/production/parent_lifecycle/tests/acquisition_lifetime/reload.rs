// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate a new Main identity before effects while retaining the authentic acquisition ledger.
use super::*;
#[test]
fn reload_preflight_keeps_original_acquisition_and_rejects_stale_or_foreign_generations()
-> Result<()> {
    let mut parent = granted()?;
    let old = identity()?;
    let original_lease = lease()?;
    let next = sugarsubstitute_contracts::startup::MainIdentity {
        launch: LaunchId::new(old.launch.get() + 1)?,
        navigation: sugarsubstitute_contracts::execution::Generation::new(
            old.navigation.get() + 1,
        )?,
        ..old
    };
    let acquisition = parent.acquisition.as_mut().ok_or("Original acquisition absent")?;
    assert!(acquisition.check_reload(old, original_lease, original_lease).is_err());
    let foreign = LeaseKey { connection: ConnectionId::new(2)?, ..original_lease };
    assert!(acquisition.check_reload(next, original_lease, foreign).is_err());
    acquisition.check_reload(next, original_lease, original_lease)?;
    assert_eq!(acquisition.header, acquisition_header()?);
    acquisition.reload(next, original_lease, original_lease)?;
    assert!(acquisition.check_reload(next, original_lease, original_lease).is_err());
    assert_eq!(acquisition.header, acquisition_header()?);
    Ok(())
}
