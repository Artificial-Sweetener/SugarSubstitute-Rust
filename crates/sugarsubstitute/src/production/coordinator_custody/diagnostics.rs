// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Attribute retained custody errors without repeated telemetry or replacing their causes.
use std::{error::Error, fmt};
/// The original failure and its actual invoking transition remain inseparable.
#[derive(Debug)]
struct TransitionFailed {
    transition: &'static str,
    cause: Box<dyn Error + Send + Sync>,
}
/// Attach a fixed developer transition label to the original owned boundary failure.
pub(super) fn failed(
    transition: &'static str,
    cause: impl Error + Send + Sync + 'static,
) -> Box<dyn Error + Send + Sync> {
    Box::new(TransitionFailed { transition, cause: Box::new(cause) })
}
impl fmt::Display for TransitionFailed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Custody transition {} failed: {}", self.transition, self.cause)
    }
}
impl Error for TransitionFailed {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
