// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A record rebuilt from its last known good copy keeps every accepted release and never a
//! candidate.
use super::*;

#[test]
fn a_restored_record_keeps_acceptance_and_strikes_but_drops_any_candidate() -> Result {
    let a = generation(1)?;
    let b = generation(2)?;
    let c = generation(3)?;
    let accepted = install(&install(&empty()?, &a)?, &b)?;
    let rejected = strike(&accepted, &c, NOW)?;
    for copy in [
        accepted.clone(),
        prepare(&accepted, &c)?,
        reload(&prepare(&accepted, &c)?.begin_trial()?)?,
        rejected.clone(),
    ] {
        let restored = reload(&copy.restored()?)?;
        assert_eq!(restored.accepted(), Some(&b));
        assert_eq!(restored.retained(), Some(&a));
        assert!(restored.candidate().is_none());
        assert!(restored.unwitnessed().is_none());
        assert_eq!(restored.strikes(), copy.strikes());
        assert_eq!(restored.rollback(), copy.rollback());
        assert_eq!(restored.revision(), copy.revision() + 1);
        assert_eq!(restored.startup_selection(), StartupSelection::Accepted(&b));
    }
    assert!(!rejected.restored()?.strikes().is_empty());
    Ok(())
}
