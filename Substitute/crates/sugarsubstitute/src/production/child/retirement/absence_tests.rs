// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Feed typed native boundary facts to the real bounded control owner; no window proof is minted.
use super::*;
#[test]
fn only_requested_current_absence_after_constructor_retirement_can_queue_main_control() -> Result<()>
{
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([3; 32]),
            epoch: CoordinatorEpoch::new([4; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let id = MainIdentity { run, launch: LaunchId::new(1)?, navigation: Generation::new(1)? };
    let mut owner = ChildShutdown::new();
    let original = ShutdownBudget::new(Moment::from_nanos(17))?;
    owner.budget = Some(original);
    assert!(owner.record_main_absence(id, id, false, false).is_err());
    owner.main_absence_pending = true;
    let successor =
        MainIdentity { launch: LaunchId::new(2)?, navigation: Generation::new(2)?, ..id };
    assert!(owner.record_main_absence(id, successor, false, false).is_err());
    assert!(owner.record_main_absence(id, id, true, false).is_err());
    assert!(owner.main_absence_pending && !owner.main_retired);
    assert!(owner.queued.iter().all(Option::is_none));
    owner.record_main_absence(id, id, false, false)?;
    assert!(!owner.main_absence_pending && owner.main_retired && !owner.main_retirement_ack);
    assert!(matches!(owner.queued[0],Some(Control::MainRetired(actual)) if actual==id));
    assert!(owner.record_main_absence(id, id, false, true).is_err());
    assert_eq!(owner.budget, Some(original));
    Ok(())
}
