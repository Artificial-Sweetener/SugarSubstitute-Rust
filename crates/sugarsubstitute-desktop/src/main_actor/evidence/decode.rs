// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind actual CDP frame/context identities and primitive diagnostics to the current native document.

use super::super::probe_diagnostics::Blocked;
use super::*;
use crate::main_readiness_types::ReadinessFault;
use serde::Deserialize;

#[derive(Deserialize)]
struct FrameEvent {
    frame: Frame,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Frame {
    id: String,
    loader_id: String,
    parent_id: Option<String>,
    url: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Detached {
    frame_id: String,
}
#[derive(Deserialize)]
struct ContextEvent {
    context: Context,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Context {
    id: i64,
    origin: String,
    aux_data: Aux,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Aux {
    frame_id: String,
    is_default: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Destroyed {
    execution_context_id: i64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Evaluation {
    result: EvaluationResult,
    exception_details: Option<serde_json::Value>,
}
#[derive(Deserialize)]
struct EvaluationResult {
    #[serde(rename = "type")]
    kind: String,
    value: Option<serde_json::Value>,
}

pub(super) fn consume(
    owner: &mut MainReadiness,
    observation: Observation<'_>,
    now: std::time::Instant,
) -> Result<Effect, DesktopError> {
    let payload = observation.payload;
    let mut effect = Effect::None;
    let result = match observation.kind {
        Kind::Frame => {
            let event: FrameEvent = parse(payload)?;
            owner.frame(
                &event.frame.id,
                &event.frame.loader_id,
                event.frame.parent_id.as_deref(),
                &event.frame.url,
            )
        }
        Kind::FrameDetached => owner.frame_detached(&parse::<Detached>(payload)?.frame_id),
        Kind::Context => {
            let event: ContextEvent = parse(payload)?;
            owner.context(
                event.context.id,
                &event.context.aux_data.frame_id,
                &event.context.origin,
                event.context.aux_data.is_default,
            )
        }
        Kind::ContextDestroyed => {
            owner.context_destroyed(parse::<Destroyed>(payload)?.execution_context_id)
        }
        Kind::ContextsCleared => {
            let _: serde_json::Value = parse(payload)?;
            owner.contexts_cleared()
        }
        Kind::Console => return console::consume(owner, payload).map(|()| Effect::None),
        Kind::ProcessFailed { .. } => {
            owner.invalidate(ReadinessFault::SetupFailed);
            Err(ReadinessFault::SetupFailed)
        }
        Kind::NavigationStarting => Ok(()),
        Kind::ReloadRequested => Err(ReadinessFault::SetupFailed),
        Kind::Navigation { succeeded } => owner.navigation_completed(succeeded),
        Kind::Command => match observation
            .command
            .ok_or_else(|| readiness_error(ReadinessFault::MalformedObservation))?
        {
            #[cfg(feature = "k1-native-qualification")]
            Command::Downloads => Err(ReadinessFault::MalformedObservation),
            Command::Runtime => {
                let _: serde_json::Value = parse(payload)?;
                effect = Effect::Hook(true);
                Ok(())
            }
            Command::Page => {
                let _: serde_json::Value = parse(payload)?;
                effect = Effect::Hook(false);
                Ok(())
            }
            Command::Probe(target) => {
                let evaluation: Evaluation = parse(payload)?;
                if evaluation.exception_details.is_some() || evaluation.result.kind != "string" {
                    return Err(readiness_error(ReadinessFault::MalformedObservation));
                }
                let text = evaluation
                    .result
                    .value
                    .as_ref()
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| readiness_error(ReadinessFault::MalformedObservation))?;
                let blocked = Blocked::decode(text)?;
                let receipt = if blocked.is_some() {
                    None
                } else {
                    Some(
                        ReadinessReceipt::parse(text.as_bytes())
                            .map_err(|cause| DesktopError::new("parse_main_receipt", cause))?,
                    )
                };
                if owner.dom_result(target, receipt, now).map_err(readiness_error)? {
                    effect = Effect::Barrier(Box::new(target));
                } else if let Some(blocked) = blocked {
                    effect = Effect::Incomplete(blocked);
                }
                Ok(())
            }
            Command::Barrier(target) => {
                let evaluation: Evaluation = parse(payload)?;
                if evaluation.exception_details.is_some()
                    || evaluation.result.kind != "number"
                    || evaluation.result.value.as_ref().and_then(serde_json::Value::as_i64)
                        != Some(0)
                {
                    return Err(readiness_error(ReadinessFault::MalformedObservation));
                }
                effect = Effect::Receipt(owner.barrier_complete(target).map_err(readiness_error)?);
                Ok(())
            }
        },
    };
    result.map_err(readiness_error)?;
    Ok(effect)
}
