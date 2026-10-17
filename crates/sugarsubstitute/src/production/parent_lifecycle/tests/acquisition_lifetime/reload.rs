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

#[test]
fn admitted_reload_reply_keeps_original_header_and_one_finite_final_transfer() -> Result<()> {
    let mut parent = actor()?;
    let h = acquisition_header()?;
    let (index, _) = parent.requests.admit(h, b"actual original reload request")?;
    parent.requests.get_mut(index)?.until = Some(moment(180));
    parent.requests.wait_for_readiness(index)?;
    assert_eq!(parent.requests.next_deadline(), None);
    parent.requests.get_mut(index)?.reply =
        Some(Reply::Reloaded { lease: lease()?, identity: identity()? });
    assert_eq!(
        parent.requests.reply_deadline(index, moment(360)).map_err(|e| e as Box<dyn Error>)?,
        moment(390)
    );
    assert_eq!(
        parent.requests.reply_deadline(index, moment(389)).map_err(|e| e as Box<dyn Error>)?,
        moment(390)
    );
    assert_eq!(parent.requests.get(index)?.header, h);
    assert_eq!(parent.requests.get(index)?.until, Some(moment(180)));
    assert_eq!(parent.requests.next_deadline(), Some(moment(390)));
    assert!(
        parent
            .requests
            .admit(
                WireHeader { deadline_qpc: h.deadline_qpc + 1, ..h },
                b"actual original reload request"
            )
            .is_err()
    );
    Ok(())
}
