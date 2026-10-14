// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode actual consumer-stage failure facts inside the sole existing generic failure slot.
use sugarsubstitute_desktop::{StartupCauseCode as Code, StartupConsumeStage as Stage};
/// Inert actual authored consumer stage and direct sealed category; no observation is inferred.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct ConsumeFailureDetail {
    pub(in super::super) stage: Stage,
    pub(in super::super) code: Code,
}
/// Require both exact fields and reject unknown stages, extensions or duplicate tokens.
pub(super) fn parse(
    mut fields: std::str::SplitAsciiWhitespace<'_>,
) -> Option<ConsumeFailureDetail> {
    let stage = match fields.next()?.strip_prefix("consume_stage=")? {
        "Retry" => Stage::Retry,
        "ExecutionDrain" => Stage::ExecutionDrain,
        "RenderDelivery" => Stage::RenderDelivery,
        "ChildDelivery" => Stage::ChildDelivery,
        "StartupEvent" => Stage::StartupEvent,
        "PolicyObservation" => Stage::PolicyObservation,
        "EffectRouting" => Stage::EffectRouting,
        "DeadlineRefresh" => Stage::DeadlineRefresh,
        _ => return None,
    };
    let code = super::startup::code(fields.next()?.strip_prefix("category=")?)?;
    if fields.next().is_some() {
        return None;
    }
    Some(ConsumeFailureDetail { stage, code })
}
#[cfg(test)]
#[path = "consume_tests.rs"]
mod tests;
