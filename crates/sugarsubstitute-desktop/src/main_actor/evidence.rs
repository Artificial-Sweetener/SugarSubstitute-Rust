// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode bounded native observations outside callbacks and perform effects after queue borrowing ends.

mod console;
mod decode;
mod probe;
use super::*;
use crate::main_events::{Kind, Observation};
use crate::main_readiness_types::ProbeTarget;
pub(super) use probe::probe_parameters;
use sugarsubstitute_contracts::startup::ReadinessReceipt;

enum Effect {
    None,
    Hook(bool),
    Barrier(Box<ProbeTarget>),
    Receipt(ReadinessReceipt),
}

impl MainActor {
    pub(super) fn evidence_step(
        &mut self,
    ) -> Result<Option<DesktopStartupObservation>, DesktopError> {
        let events = self.events()?.clone();
        let mut effect = Effect::None;
        // The pure decoder/readiness owner performs no native calls while queue storage is borrowed.
        let consumed = events.consume(|observation| {
            match decode_observation(self.readiness()?, observation) {
                Ok(decoded) => effect = decoded,
                Err((cause, kind)) => {
                    self.failure_observation = kind;
                    return Err(cause);
                }
            }
            Ok(())
        })?;
        if !consumed {
            return Ok(None);
        }
        let result = match effect {
            Effect::None => None,
            Effect::Hook(runtime) => {
                if self.admission.hook_completed(runtime)? {
                    self.admission.navigation_admitted()?;
                    self.readiness()?.navigation_started().map_err(readiness_error)?;
                    self.receivers()?.begin_navigation()?;
                    let url = crate::main_navigation::root_url(self.request()?.origin());
                    self.view()?
                        .load_url(&url)
                        .map_err(|cause| DesktopError::new("navigate_main_frontend", cause))?;
                    self.navigations += 1;
                    #[cfg(feature = "k1-native-qualification")]
                    tracing::info!(operation="main_navigation_started", identity=?self.identity(),
                        "Actual local frontend navigation dispatched after hooks");
                    Some(DesktopStartupObservation::NavigationStarted(self.identity()))
                } else {
                    None
                }
            }
            Effect::Barrier(target) => {
                // Same native CDP session and exact execution context; later health observations
                // remain queued before this completion can transfer evidence to the application.
                let parameters = format!(
                    "{{\"expression\":\"0\",\"contextId\":{},\"returnByValue\":true}}",
                    target.context
                );
                self.receivers()?.call(
                    Command::Barrier(*target),
                    "Runtime.evaluate",
                    &parameters,
                )?;
                None
            }
            Effect::Receipt(receipt) => Some(DesktopStartupObservation::FrontendReceipt {
                identity: self.identity(),
                receipt,
            }),
        };
        events.rewake();
        Ok(result)
    }
}

/// Preserve the actual callback kind together with the unchanged decoder failure.
fn decode_observation(
    owner: &mut MainReadiness,
    observation: Observation<'_>,
) -> Result<Effect, (DesktopError, &'static str)> {
    let kind = observation_kind(observation.kind, observation.command);
    decode::consume(owner, observation).map_err(|cause| (cause, kind))
}

/// Map actual queued callback identities without retaining any native payload.
fn observation_kind(kind: Kind, command: Option<Command>) -> &'static str {
    match kind {
        Kind::Frame => "Frame",
        Kind::FrameDetached => "FrameDetached",
        Kind::Context => "Context",
        Kind::ContextDestroyed => "ContextDestroyed",
        Kind::ContextsCleared => "ContextsCleared",
        Kind::Console => "Console",
        Kind::NavigationStarting => "NavigationStarting",
        Kind::Navigation { .. } => "Navigation",
        Kind::ProcessFailed { .. } => "ProcessFailed",
        Kind::Command => match command {
            Some(Command::Runtime) => "Runtime",
            Some(Command::Page) => "Page",
            Some(Command::Probe(_)) => "Probe",
            Some(Command::Barrier(_)) => "Barrier",
            None => "Command",
        },
    }
}

/// Check shape before allocating decoded native JSON values; input stays under its callback byte bound.
fn bounded_json(payload: &str) -> Result<(), DesktopError> {
    let mut depth = 0_usize;
    let mut tokens = 0_usize;
    let mut string = false;
    let mut escaped = false;
    for byte in payload.bytes() {
        if string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                string = false;
            }
        } else {
            match byte {
                b'"' => {
                    string = true;
                    tokens += 1;
                }
                b'{' | b'[' => {
                    depth += 1;
                    tokens += 1;
                }
                b'}' | b']' => depth = depth.saturating_sub(1),
                b',' => tokens += 1,
                _ => {}
            }
            if depth > 16 || tokens > 512 {
                return Err(DesktopError::message(
                    "decode_main_observation",
                    "native JSON structure bound exceeded",
                ));
            }
        }
    }
    if payload.len() > crate::main_events::PAYLOAD_BYTES {
        return Err(DesktopError::message(
            "decode_main_observation",
            "native JSON byte bound exceeded",
        ));
    }
    Ok(())
}

fn parse<T: serde::de::DeserializeOwned>(payload: &str) -> Result<T, DesktopError> {
    bounded_json(payload)?;
    serde_json::from_str(payload)
        .map_err(|cause| DesktopError::new("decode_main_observation", cause))
}

#[cfg(test)]
#[path = "evidence_tests.rs"]
mod tests;
