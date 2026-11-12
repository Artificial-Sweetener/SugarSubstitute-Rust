// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Interpret main-surface closure against its actual completed reveal.

use crate::{DesktopError, DesktopStartupObservation};

/// An unrevealed main surface cannot stand in for the user's visible splash.
/// Its loss fails the current attempt through checked retirement and splash retry.
/// After reveal, native and browser close requests retire the whole application.
/// Callers suppress observations belonging to an already-retiring actor.
pub(super) fn observe(revealed: bool) -> Result<Option<DesktopStartupObservation>, DesktopError> {
    if revealed {
        Ok(Some(DesktopStartupObservation::CloseRequested))
    } else {
        Err(DesktopError::message(
            "main_closed_before_reveal",
            "main surface closed before its ready window was revealed",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_main_closure_is_a_recoverable_attempt_failure() {
        assert!(
            observe(false)
                .is_err_and(|cause| cause.to_string().contains("main_closed_before_reveal"))
        );
    }

    #[test]
    fn revealed_main_closure_preserves_normal_application_exit() -> Result<(), DesktopError> {
        assert!(matches!(observe(true)?, Some(DesktopStartupObservation::CloseRequested)));
        Ok(())
    }
}
