// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compact acknowledgement diagnostics survive the bounded desktop stderr relay.
//! These records describe consumed replies; they never authorize update activation.
use sugarsubstitute_contracts::{
    execution::Generation,
    startup::{LaunchId, MainIdentity},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Inert attempt coordinates, correlated with the run by the authenticated output owner.
pub(super) struct Record {
    pub(super) launch: LaunchId,
    pub(super) navigation: Generation,
}
impl Record {
    /// Retain only bounded attempt coordinates after the authenticated reply was consumed.
    pub(super) fn new(identity: MainIdentity) -> Self {
        Self { launch: identity.launch, navigation: identity.navigation }
    }

    /// Emit the shared schema without paths, raw payloads or an oversized run identity.
    pub(super) fn emit(self) {
        tracing::info!(
            operation = "startup_readiness_acknowledged",
            schema = 1,
            launch = self.launch.get(),
            navigation = self.navigation.get()
        );
    }

    /// Decode exact schema-one fields from this producer. Unknown or extra data is discarded.
    pub(super) fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > 512 {
            return None;
        }
        let line = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
        let (prefix, fields) = line.split_once("operation=\"startup_readiness_acknowledged\" ")?;
        if prefix.contains("operation=")
            || !prefix.ends_with("sugarsubstitute::production::startup_readiness_diagnostic: ")
        {
            return None;
        }
        let mut fields = fields.split_ascii_whitespace();
        if fields.next()? != "schema=1" {
            return None;
        }
        let launch = LaunchId::new(number(fields.next()?, "launch=")?).ok()?;
        let navigation = Generation::new(number(fields.next()?, "navigation=")?).ok()?;
        if fields.next().is_some() {
            return None;
        }
        Some(Self { launch, navigation })
    }
}
/// Reject noncanonical or overflowing coordinates before constructing their typed identities.
fn number(field: &str, prefix: &str) -> Option<u64> {
    let raw = field.strip_prefix(prefix)?;
    if raw.is_empty() || raw.starts_with('0') || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    raw.parse().ok()
}

#[cfg(test)]
#[path = "startup_readiness_diagnostic/tests.rs"]
mod tests;
