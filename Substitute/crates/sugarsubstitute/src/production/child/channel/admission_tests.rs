// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify the actual channel fence using deterministic terminal metadata.
use super::*;

#[test]
fn every_matching_terminal_kind_fences_before_typed_completion_transfers()
-> Result<(), TerminalConflict> {
    for kind in [
        TerminalKind::Success,
        TerminalKind::Failure,
        TerminalKind::Panicked,
        TerminalKind::Cancelled,
        TerminalKind::TimedOut,
        TerminalKind::Coalesced,
    ] {
        let mut admission = ChannelAdmission::new();
        assert!(admission.open());
        admission.observe_terminal(kind)?;
        assert!(!admission.open());
        assert_eq!(admission.terminal(), Some(kind));
    }
    Ok(())
}

#[test]
fn repeated_matching_notice_does_not_renew_admission() -> Result<(), TerminalConflict> {
    let mut admission = ChannelAdmission::new();
    admission.observe_terminal(TerminalKind::Failure)?;
    admission.observe_terminal(TerminalKind::Failure)?;
    assert!(!admission.open());
    assert_eq!(admission.terminal(), Some(TerminalKind::Failure));
    Ok(())
}

#[test]
fn changed_classification_preserves_the_first_notice_and_fence() -> Result<(), TerminalConflict> {
    let mut admission = ChannelAdmission::new();
    admission.observe_terminal(TerminalKind::Cancelled)?;
    assert_eq!(admission.observe_terminal(TerminalKind::Success), Err(TerminalConflict));
    assert!(!admission.open());
    assert_eq!(admission.terminal(), Some(TerminalKind::Cancelled));
    Ok(())
}

#[test]
fn explicit_close_stays_closed_before_and_after_terminal_metadata() -> Result<(), TerminalConflict>
{
    let mut admission = ChannelAdmission::new();
    admission.close();
    admission.close();
    assert!(!admission.open());
    assert_eq!(admission.terminal(), None);
    admission.observe_terminal(TerminalKind::Success)?;
    admission.close();
    assert!(!admission.open());
    assert_eq!(admission.terminal(), Some(TerminalKind::Success));
    Ok(())
}
