// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bounded native launch-transfer messages; bytes alone never grant creation authority.
use super::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, ffi::OsString, path::Path};
use sugarsubstitute_contracts::application_launch::{
    INSTALLED_LAUNCH_ARGUMENT, TRIAL_LAUNCH_ARGUMENT,
};

/// Exact maximum before parsing an untrusted native transfer message.
pub(super) const MAX_BYTES: usize = sugarsubstitute_execution::activation_channel::MAX_FRAME_BYTES;
/// Bounds both native duplication work and retained file/directory handles.
pub(super) const MAX_PINS: usize = 65536;

/// An authenticated engine supplies exactly one supported installed-application entry.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Entry {
    /// Accepted selection read custody; this never grants application-run ownership.
    Installed { selection: u64 },
    /// Exact original run exclusion and immutable trial snapshot.
    Trial { run: u64, snapshot: u64 },
}
impl Entry {
    /// Preserve the native entry's fixed handle order across duplication.
    pub(super) fn handles(self) -> Vec<u64> {
        match self {
            Self::Installed { selection } => vec![selection],
            Self::Trial { run, snapshot } => vec![run, snapshot],
        }
    }
    /// Compose arguments from receiver-owned duplicates, never sender-process handle numbers.
    pub(super) fn arguments(self, handles: &[usize]) -> Result<Vec<OsString>, ClaimError> {
        if handles.len() != self.handles().len() {
            return Err(ClaimError::invalid("launch_transfer_entry_handles"));
        }
        let prefix = match self {
            Self::Installed { .. } => INSTALLED_LAUNCH_ARGUMENT,
            Self::Trial { .. } => TRIAL_LAUNCH_ARGUMENT,
        };
        Ok(std::iter::once(prefix.into())
            .chain(handles.iter().map(|handle| handle.to_string().into()))
            .collect())
    }
}

/// Source-process handle identities remain untrusted until duplicated from the exact retained engine.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub(super) version: u32,
    pub(super) id: [u8; 32],
    pub(super) executable: String,
    pub(super) directory: String,
    pub(super) entry: Entry,
    #[serde(deserialize_with = "bounded_pins")]
    pub(super) pins: Vec<u64>,
}

/// Enforce the count while decoding, before a hostile short-number array can grow its allocation.
fn bounded_pins<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<u64>, D::Error> {
    struct Pins;
    impl<'de> serde::de::Visitor<'de> for Pins {
        type Value = Vec<u64>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a bounded native pin inventory")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> Result<Self::Value, A::Error> {
            let mut pins = Vec::new();
            while let Some(pin) = sequence.next_element()? {
                if pins.len() == MAX_PINS {
                    return Err(serde::de::Error::custom("Native pin inventory exceeds its bound"));
                }
                pins.push(pin);
            }
            Ok(pins)
        }
    }
    deserializer.deserialize_seq(Pins)
}
impl Request {
    /// Bound the entire frame before parsing and require the receiver's original request identity.
    pub(super) fn decode(bytes: &[u8], id: [u8; 32]) -> Result<Self, ClaimError> {
        if bytes.len() > MAX_BYTES {
            return Err(ClaimError::invalid("launch_transfer_frame_bound"));
        }
        let request: Self = serde_json::from_slice(bytes)
            .map_err(|cause| ClaimError::new("launch_transfer_decode", cause))?;
        request.validate()?;
        if request.id != id {
            return Err(ClaimError::invalid("launch_transfer_identity"));
        }
        Ok(request)
    }
    /// Validate before encoding so locally produced and remotely received requests share one rule.
    pub(super) fn encode(&self) -> Result<Vec<u8>, ClaimError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|cause| ClaimError::new("launch_transfer_encode", cause))?;
        if bytes.len() > MAX_BYTES {
            return Err(ClaimError::invalid("launch_transfer_frame_bound"));
        }
        Ok(bytes)
    }
    /// Check bounded lexical command shape and distinct non-pseudo native identities, without I/O.
    fn validate(&self) -> Result<(), ClaimError> {
        if self.version != 1
            || self.id == [0; 32]
            || self.pins.is_empty()
            || self.pins.len() > MAX_PINS
        {
            return Err(ClaimError::invalid("launch_transfer_shape"));
        }
        if self.executable.len() > 16384 || self.directory.len() > 16384 {
            return Err(ClaimError::invalid("launch_transfer_paths"));
        }
        let mut seen = BTreeSet::new();
        for raw in self.entry.handles().into_iter().chain(self.pins.iter().copied()) {
            let handle = usize::try_from(raw)
                .map_err(|cause| ClaimError::new("launch_transfer_handle", cause))?;
            if handle == 0 || handle > isize::MAX as usize || !seen.insert(handle) {
                return Err(ClaimError::invalid("launch_transfer_handle"));
            }
        }
        let handles = self.entry.handles().into_iter().map(|raw| raw as usize).collect::<Vec<_>>();
        self.command(&handles)?;
        Ok(())
    }
    /// Use the ordinary typed command boundary with only the supported entry's remapped handles.
    pub(super) fn command(&self, handles: &[usize]) -> Result<WindowsLaunchCommand, ClaimError> {
        WindowsLaunchCommand::from_os(
            Path::new(&self.executable),
            Path::new(&self.directory),
            &self.entry.arguments(handles)?,
        )
        .map_err(|cause| ClaimError::new("launch_transfer_command", cause))
    }
}

/// Observed creation disposition; acceptance and application health remain separate protocols.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Disposition {
    NotCreated,
    Created,
    Indeterminate,
}

/// One correlated terminal response from the process that actually owns native creation.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Response {
    version: u32,
    id: [u8; 32],
    disposition: Disposition,
}

/// Require the exact offered request before changing the engine's trial-recovery eligibility.
pub(super) fn response(bytes: &[u8], id: [u8; 32]) -> Result<CreationOutcome, ClaimError> {
    if bytes.len() > 1024 {
        return Err(ClaimError::invalid("launch_transfer_response_bound"));
    }
    let response: Response = serde_json::from_slice(bytes)
        .map_err(|cause| ClaimError::new("launch_transfer_response_decode", cause))?;
    if response.version != 1 || response.id != id || id == [0; 32] {
        return Err(ClaimError::invalid("launch_transfer_response_identity"));
    }
    Ok(match response.disposition {
        Disposition::NotCreated => CreationOutcome::NotCreated,
        Disposition::Created => CreationOutcome::Created,
        Disposition::Indeterminate => CreationOutcome::Indeterminate,
    })
}

/// Serialize only an actually observed native result for the original request.
pub(super) fn result(id: [u8; 32], outcome: CreationOutcome) -> Result<Vec<u8>, ClaimError> {
    if id == [0; 32] {
        return Err(ClaimError::invalid("launch_transfer_response_identity"));
    }
    let disposition = match outcome {
        CreationOutcome::NotCreated => Disposition::NotCreated,
        CreationOutcome::Created => Disposition::Created,
        CreationOutcome::Indeterminate => Disposition::Indeterminate,
    };
    serde_json::to_vec(&Response { version: 1, id, disposition })
        .map_err(|cause| ClaimError::new("launch_transfer_response_encode", cause))
}

#[cfg(test)]
mod tests;
