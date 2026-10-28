// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual bounded startup consumer failures without observation inference.
use crate::StartupCauseCode;
use std::{error::Error, fmt};

/// The authored consumer operation that actually returned the first error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupConsumeStage {
    /// User retry preparation failed.
    Retry,
    /// The actual execution owner's completion drain failed.
    ExecutionDrain,
    /// Governed render completion delivery failed first.
    RenderDelivery,
    /// Actual child completion delivery failed.
    ChildDelivery,
    /// The application-owned canvas could not advance its retained work.
    CanvasDelivery,
    /// The actual child could not transfer its retained startup event.
    StartupEvent,
    /// Application policy rejected the actual startup event.
    PolicyObservation,
    /// Dispatching the actual application effect failed.
    EffectRouting,
    /// The existing deadline could not be projected for native waiting.
    DeadlineRefresh,
}

/// Own the unchanged returned error and inert consumer-stage facts.
pub struct StartupConsumeFailure {
    stage: StartupConsumeStage,
    code: StartupCauseCode,
    cause: Box<dyn Error>,
}
impl StartupConsumeFailure {
    /// Transfer the actual returned Box without inspecting arbitrary callbacks.
    /// Already retained first-stage facts and their original cause remain unchanged.
    pub fn retain(
        stage: StartupConsumeStage,
        code: StartupCauseCode,
        cause: Box<dyn Error>,
    ) -> Box<dyn Error> {
        if cause.downcast_ref::<Self>().is_some() {
            return cause;
        }
        Box::new(Self { stage, code, cause })
    }
    /// Return the actual first authored consumer stage.
    pub const fn stage(&self) -> StartupConsumeStage {
        self.stage
    }
    /// Return the direct sealed cause category supplied by its authoritative owner.
    pub const fn code(&self) -> StartupCauseCode {
        self.code
    }
    /// Borrow the same original Box payload without walking its sources.
    pub fn original(&self) -> &(dyn Error + 'static) {
        self.cause.as_ref()
    }
}
impl fmt::Debug for StartupConsumeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StartupConsumeFailure")
    }
}
impl fmt::Display for StartupConsumeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Original startup consumer operation failed")
    }
}
impl Error for StartupConsumeFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
