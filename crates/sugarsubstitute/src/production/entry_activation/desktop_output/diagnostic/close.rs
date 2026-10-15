// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate closed finite close transitions inside the original bounded output decoder.
use crate::production::close_diagnostic::{
    BudgetStage, CloseTrace, Control, ControlStage, DeadlineSource, Prepared,
};
use sugarsubstitute_application::startup::StartupPhase;
use sugarsubstitute_contracts::lifecycle::RunKey;
/// Decode a canonical unsigned scalar without accepting extensions.
fn number(field: &str, prefix: &str) -> Option<u64> {
    let value = field.strip_prefix(prefix)?;
    if value.is_empty()
        || !value.bytes().all(|b| b.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return None;
    }
    value.parse().ok()
}
/// Accept only the closed startup-phase vocabulary.
fn phase(field: &str) -> Option<Option<StartupPhase>> {
    Some(match field.strip_prefix("phase=")? {
        "None" => None,
        "Some(SplashFirstFrame)" => Some(StartupPhase::SplashFirstFrame),
        "Some(PreparingConnection)" => Some(StartupPhase::PreparingConnection),
        "Some(ServerReady)" => Some(StartupPhase::ServerReady),
        "Some(WebviewLoading)" => Some(StartupPhase::WebviewLoading),
        "Some(FrontendReady)" => Some(StartupPhase::FrontendReady),
        "Some(RevealingMain)" => Some(StartupPhase::RevealingMain),
        "Some(Running)" => Some(StartupPhase::Running),
        "Some(Failed)" => Some(StartupPhase::Failed),
        "Some(Cancelling)" => Some(StartupPhase::Cancelling),
        "Some(Stopped)" => Some(StartupPhase::Stopped),
        _ => return None,
    })
}
/// Reject extra, duplicate, unknown, truncated or noncanonical fields before passive relay.
pub(super) fn parse(bytes: &[u8]) -> Option<CloseTrace> {
    if bytes.len() > 512 {
        return None;
    }
    let line = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
    let (prefix, body) = line.split_once("operation=\"")?;
    if prefix.contains("operation=") {
        return None;
    }
    let mut fields;
    let record = if let Some(body) = body.strip_prefix("child_close_control\" ") {
        fields = body.split_ascii_whitespace();
        let stage = match fields.next()? {
            "stage=Prepared" => ControlStage::Prepared,
            "stage=Admitted" => ControlStage::Admitted,
            "stage=ReplyReceived" => ControlStage::ReplyReceived,
            "stage=ReplyConsumed" => ControlStage::ReplyConsumed,
            _ => return None,
        };
        let control = match fields.next()? {
            "control=MainRetired" => Control::MainRetired,
            "control=Release" => Control::Release,
            "control=ExitIntent" => Control::ExitIntent,
            "control=ExitCompleted" => Control::ExitCompleted,
            _ => return None,
        };
        let request = number(fields.next()?, "request=")?;
        let deadline_qpc = i64::try_from(number(fields.next()?, "deadline_qpc=")?).ok()?;
        if request == 0 || deadline_qpc == 0 {
            return None;
        }
        let prepared = if stage == ControlStage::Prepared {
            let original_ns = number(fields.next()?, "original_ns=")?;
            let source = match fields.next()? {
                "deadline_source=ShutdownGrace" => DeadlineSource::ShutdownGrace,
                "deadline_source=ShutdownTotal" => DeadlineSource::ShutdownTotal,
                "deadline_source=AttemptCleanup" => DeadlineSource::AttemptCleanup,
                "deadline_source=ControlInterval" => DeadlineSource::ControlInterval,
                _ => return None,
            };
            let phase = phase(fields.next()?)?;
            let attempt_present = super::boolean(fields.next()?, "attempt_present=")?;
            let attempt_complete = super::boolean(fields.next()?, "attempt_complete=")?;
            let shutdown_budget_present =
                super::boolean(fields.next()?, "shutdown_budget_present=")?;
            if attempt_complete && !attempt_present {
                return None;
            }
            Some(Prepared {
                original_ns,
                source,
                phase,
                attempt_present,
                attempt_complete,
                shutdown_budget_present,
            })
        } else {
            None
        };
        CloseTrace::Control { stage, control, request, deadline_qpc, prepared }
    } else if let Some(body) = body.strip_prefix("child_shutdown_budget\" ") {
        fields = body.split_ascii_whitespace();
        let stage = match fields.next()? {
            "stage=CapturePending" => BudgetStage::CapturePending,
            "stage=LocalMeasured" => BudgetStage::LocalMeasured,
            "stage=MeasurementFailed" => BudgetStage::MeasurementFailed,
            "stage=CaptureLatched" => BudgetStage::CaptureLatched,
            "stage=CaptureMeasurementFailed" => BudgetStage::CaptureMeasurementFailed,
            _ => return None,
        };
        let moments = if matches!(stage, BudgetStage::LocalMeasured | BudgetStage::CaptureLatched) {
            let values = [
                number(fields.next()?, "started_ns=")?,
                number(fields.next()?, "graceful_ns=")?,
                number(fields.next()?, "settle_ns=")?,
            ];
            if values[0] > values[1] || values[1] > values[2] {
                return None;
            }
            Some(values)
        } else {
            None
        };
        CloseTrace::Budget { stage, moments }
    } else {
        return None;
    };
    if fields.next().is_some() {
        return None;
    }
    Some(record)
}
/// Run comes from the actual authenticated output batch, never from child text.
pub(in super::super) fn relay(run: RunKey, close: CloseTrace) {
    tracing::info!(operation="desktop_child_close",run=?run,close=?close,
        "Observed owned child's closed lifecycle diagnostic");
}
#[cfg(test)]
#[path = "close_tests.rs"]
mod tests;
