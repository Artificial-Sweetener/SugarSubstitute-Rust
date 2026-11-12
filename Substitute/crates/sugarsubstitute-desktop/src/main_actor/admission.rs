// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Guard two independent native validation fences before frontend effects.

use crate::{DesktopError, MainIdentity};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    ConstructingEnvironment,
    Environment,
    ConstructingController,
    Browser,
    Hooks,
    Frontend,
    Retiring,
}

/// Presentation admission contains no profile lease, kernel membership or startup policy.
/// The trusted composing owner alone supplies commands after its independent validations.
pub(super) struct Admission {
    identity: MainIdentity,
    phase: Phase,
    browser_process_id: Option<u32>,
    runtime: bool,
    page: bool,
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;

impl Admission {
    pub(super) fn new(identity: MainIdentity) -> Self {
        Self {
            identity,
            phase: Phase::ConstructingEnvironment,
            browser_process_id: None,
            runtime: false,
            page: false,
        }
    }
    pub(super) fn identity(&self) -> MainIdentity {
        self.identity
    }
    /// Return the original actually observed controller process for qualification rechecks.
    #[cfg(feature = "k1-native-qualification")]
    pub(super) fn browser_process_id(&self) -> Option<u32> {
        self.browser_process_id
    }
    pub(super) fn closing(&self) -> bool {
        self.phase == Phase::Retiring
    }
    pub(super) fn close(&mut self) {
        self.phase = Phase::Retiring;
    }

    pub(super) fn environment_observed(&mut self) -> Result<(), DesktopError> {
        self.advance(Phase::ConstructingEnvironment, Phase::Environment, "observe_main_environment")
    }
    pub(super) fn authorize_environment(
        &mut self,
        identity: MainIdentity,
    ) -> Result<(), DesktopError> {
        self.check(identity)?;
        self.advance(
            Phase::Environment,
            Phase::ConstructingController,
            "authorize_main_environment",
        )
    }
    pub(super) fn controller_observed(&mut self, pid: u32) -> Result<(), DesktopError> {
        if pid == 0 {
            return Err(DesktopError::message(
                "observe_main_browser",
                "native browser identity is missing",
            ));
        }
        self.advance(Phase::ConstructingController, Phase::Browser, "observe_main_browser")?;
        self.browser_process_id = Some(pid);
        Ok(())
    }
    pub(super) fn authorize_browser(&mut self, identity: MainIdentity) -> Result<(), DesktopError> {
        self.check(identity)?;
        self.advance(Phase::Browser, Phase::Hooks, "authorize_main_browser")
    }
    pub(super) fn hook_completed(&mut self, runtime: bool) -> Result<bool, DesktopError> {
        if self.phase != Phase::Hooks {
            return Err(DesktopError::message(
                "enable_main_hooks",
                "hook completion is not current",
            ));
        }
        let done = if runtime { &mut self.runtime } else { &mut self.page };
        if *done {
            return Err(DesktopError::message("enable_main_hooks", "duplicate hook completion"));
        }
        *done = true;
        Ok(self.runtime && self.page)
    }
    pub(super) fn navigation_admitted(&mut self) -> Result<(), DesktopError> {
        if !self.runtime || !self.page {
            return Err(DesktopError::message(
                "navigate_main_frontend",
                "native hooks are not enabled",
            ));
        }
        self.advance(Phase::Hooks, Phase::Frontend, "navigate_main_frontend")
    }
    pub(super) fn frontend(&self, identity: MainIdentity) -> Result<(), DesktopError> {
        self.check(identity)?;
        if self.phase == Phase::Frontend {
            Ok(())
        } else {
            Err(DesktopError::message("main_frontend_effect", "frontend admission is closed"))
        }
    }
    /// Borrow actual close/identity admission before a passive expiry diagnostic.
    /// Repeated retirement has no second diagnosis; early or foreign requests reject.
    pub(super) fn timeout_diagnostic_allowed(
        &self,
        identity: MainIdentity,
    ) -> Result<bool, DesktopError> {
        self.check(identity)?;
        if self.closing() {
            return Ok(false);
        }
        self.frontend(identity)?;
        Ok(true)
    }
    fn check(&self, identity: MainIdentity) -> Result<(), DesktopError> {
        if identity == self.identity {
            Ok(())
        } else {
            Err(DesktopError::message(
                "validate_main_identity",
                "main identity is foreign or stale",
            ))
        }
    }
    fn advance(
        &mut self,
        expected: Phase,
        next: Phase,
        operation: &'static str,
    ) -> Result<(), DesktopError> {
        if self.phase != expected {
            return Err(DesktopError::message(
                operation,
                "native effect is out of order or closed",
            ));
        }
        self.phase = next;
        Ok(())
    }
}
