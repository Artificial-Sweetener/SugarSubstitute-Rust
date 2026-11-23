// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! One closed record per live preview frame the canvas offers, relayed to the application log.
//!
//! A live preview passes through loading, CPU preparation and the display pacer before it is
//! offered for presentation. Each offered frame records how long each of those steps took, so a
//! slow preview names the step that holds it.
use std::time::{Duration, Instant};

/// When one preview candidate began and finished each step, read from the monotonic clock.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PreviewTiming {
    started: Instant,
    loaded: Option<Instant>,
    prepared: Option<Instant>,
}

impl PreviewTiming {
    /// Begin timing a candidate taken from the pending slot.
    pub(crate) fn start() -> Self {
        Self { started: Instant::now(), loaded: None, prepared: None }
    }

    /// The candidate's source finished loading.
    pub(crate) fn loaded(&mut self) {
        self.loaded.get_or_insert_with(Instant::now);
    }

    /// The candidate's CPU frame finished preparing.
    pub(crate) fn prepared(&mut self) {
        self.prepared.get_or_insert_with(Instant::now);
    }

    /// The record of a candidate offered for presentation now.
    pub(crate) fn offered(self, token: u64) -> Record {
        let now = Instant::now();
        let loaded = self.loaded.unwrap_or(now);
        let prepared = self.prepared.unwrap_or(now).max(loaded);
        let micros = |value: Duration| u64::try_from(value.as_micros()).unwrap_or(u64::MAX);
        Record {
            token,
            load: micros(loaded.saturating_duration_since(self.started)),
            prepare: micros(prepared.saturating_duration_since(loaded)),
            offer: micros(now.saturating_duration_since(prepared)),
        }
    }
}

/// One offered preview frame's step durations in whole microseconds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Record {
    pub(crate) token: u64,
    pub(crate) load: u64,
    pub(crate) prepare: u64,
    pub(crate) offer: u64,
}

impl Record {
    /// Write this frame on the child's diagnostic output.
    pub(crate) fn emit(self) {
        tracing::info!(
            operation = "canvas_preview_offered",
            schema = 1,
            frame = self.token,
            load_us = self.load,
            prepare_us = self.prepare,
            offer_us = self.offer
        );
    }

    /// Accept exactly the schema-one line `emit` writes, within the child-output scratch.
    #[cfg(feature = "windows-native-host")]
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > 512 {
            return None;
        }
        let text = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
        let (prefix, fields) = text.split_once("operation=\"canvas_preview_offered\" ")?;
        if prefix.contains("operation=")
            || !prefix.ends_with("sugarsubstitute::canvas_preview_diagnostic: ")
        {
            return None;
        }
        let mut fields = fields.split_ascii_whitespace();
        let mut field = |name: &str| fields.next()?.strip_prefix(name)?.strip_prefix('=');
        if field("schema")? != "1" {
            return None;
        }
        let token = number(field("frame")?)?;
        let load = number(field("load_us")?)?;
        let prepare = number(field("prepare_us")?)?;
        let offer = number(field("offer_us")?)?;
        if fields.next().is_some() {
            return None;
        }
        Some(Self { token, load, prepare, offer })
    }

    /// Write the relayed frame timing to the application log under the child's run.
    #[cfg(feature = "windows-native-host")]
    pub(crate) fn relay(self, run: sugarsubstitute_contracts::lifecycle::RunKey) {
        let ms = |value: u64| value as f64 / 1000.0;
        tracing::info!(operation = "desktop_child_canvas_preview_offered", run = ?run,
            frame = self.token, load_ms = ms(self.load), prepare_ms = ms(self.prepare),
            offer_ms = ms(self.offer), "A live preview frame was offered for presentation");
    }
}

/// Refuse signs, padding and overflow at the passive wire boundary.
#[cfg(feature = "windows-native-host")]
fn number(value: &str) -> Option<u64> {
    if value.is_empty()
        || value.len() > 1 && value.starts_with('0')
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}

#[cfg(all(test, feature = "windows-native-host"))]
#[path = "canvas_preview_diagnostic/tests.rs"]
mod tests;
