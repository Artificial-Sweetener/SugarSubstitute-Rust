// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode actual consumer-stage failure facts inside the sole existing generic failure slot.
use sugarsubstitute_desktop::{
    StartupCauseCode as Code, StartupChildStep as Step, StartupConsumeStage as Stage,
};
/// Inert actual authored consumer stage and direct sealed category; no observation is inferred.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct ConsumeFailureDetail {
    pub(in super::super) stage: Stage,
    pub(in super::super) code: Code,
    /// The production child step that returned the cause, when the child named one.
    pub(in super::super) step: Option<Step>,
}
/// Require both exact fields, accept one trailing child step, and reject unknown stages,
/// extensions or duplicate tokens.
pub(super) fn parse(
    mut fields: std::str::SplitAsciiWhitespace<'_>,
) -> Option<ConsumeFailureDetail> {
    let stage = Stage::parse(fields.next()?.strip_prefix("consume_stage=")?)?;
    let code = Code::parse(fields.next()?.strip_prefix("category=")?)?;
    let step = match fields.next() {
        Some(field) => Some(Step::parse(field.strip_prefix("child_step=")?)?),
        None => None,
    };
    if fields.next().is_some() {
        return None;
    }
    Some(ConsumeFailureDetail { stage, code, step })
}
#[cfg(test)]
#[path = "consume_tests.rs"]
mod tests;
