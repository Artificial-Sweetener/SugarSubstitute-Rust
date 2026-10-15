// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve actual cause allocation and claim before any retired Record is dropped.
use super::*;
use crate::production::parent_lifecycle::causes::BackendCauses;
/// Only a genuine retired prior role can surrender a mounted Parent's causes.
/// Prelaunch successors carry the already-owned box without creating a Parent or a bootstrap.
pub(super) fn before_replace(
    previous: &mut Record,
    current: &mut Record,
    restart: bool,
) -> Result<()> {
    if !restart
        || !previous.closed
        || previous.run.owner != current.run.owner
        || previous.run.instance != current.run.instance
        || previous.run.generation >= current.run.generation
        || current.parent.is_some()
        || current.retained_causes.is_some()
    {
        return Err("Actual restart cause handoff phase mismatch".into());
    }
    if let Some(parent) = previous.parent.as_mut() {
        if previous.retained_causes.is_some() {
            return Err("Cause owner duplicated across role containers".into());
        }
        current.retained_causes = parent.take_retired_backend_causes()?;
    } else {
        BackendCauses::transfer(&mut previous.retained_causes, &mut current.retained_causes)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "cause_transfer/tests.rs"]
mod tests;
