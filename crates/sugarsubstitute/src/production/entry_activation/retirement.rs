// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish run retirement only after actual profile, family and pipe owner checks.
use super::launch::DesktopStart;
use crate::production::parent_lifecycle::ParentLifecycle;
use std::error::Error;
use sugarsubstitute_supervision::{FamilyBinding, WindowsProcessRegistry};

#[path = "retirement/receipt.rs"]
mod receipt;
pub(crate) use receipt::RetiredDesktop;
/// Preserve unsettled records and profile custody; absence is never settlement evidence.
pub(super) fn retire(
    start: &mut DesktopStart,
    parent: &ParentLifecycle,
    registry: &mut WindowsProcessRegistry,
) -> Result<Option<RetiredDesktop>, Box<dyn Error>> {
    if !start.profile.snapshot().released
        || start.profile.snapshot().pending
        || !parent.native_retired()
    {
        return Ok(None);
    }
    let Some(registration) = parent.retired_registration(start.run, start.connection)? else {
        return Ok(None);
    };
    let Some(ticket) = start.ticket else { return Ok(None) };
    if ticket.binding() != FamilyBinding::Desktop(start.run) {
        return Err("Desktop retirement binding mismatch".into());
    }
    let actual = registry.snapshot(ticket.process())?;
    if actual.binding != FamilyBinding::Desktop(start.run) || !actual.settled {
        return Ok(None);
    }
    // The registry rejects unread facts or any remaining native owner/job.
    registry.retire_settled(ticket.process())?;
    start.ticket = None;
    Ok(Some(RetiredDesktop::new(registration)))
}
