// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode the first native retirement reason without interpreting arbitrary child text.
use sugarsubstitute_desktop::MainRetirementReason;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct MainRetirement {
    pub(in super::super) launch: u64,
    pub(in super::super) navigation: u64,
    pub(in super::super) reason: MainRetirementReason,
}
pub(super) fn parse(bytes: &[u8]) -> Option<MainRetirement> {
    if bytes.len() > 512 {
        return None;
    }
    let line = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
    let (prefix, fields) = line.split_once("operation=\"main_retirement_requested\" ")?;
    if prefix.contains("operation=") {
        return None;
    }
    let mut fields = fields.split_ascii_whitespace();
    let launch = super::main::identity(fields.next()?.strip_prefix("launch=")?)?;
    let navigation = super::main::identity(fields.next()?.strip_prefix("navigation=")?)?;
    let reason = match fields.next()?.strip_prefix("reason=")? {
        "AttemptCleanup" => MainRetirementReason::AttemptCleanup,
        "FrontendTimeout" => MainRetirementReason::FrontendTimeout,
        "FrontendReload" => MainRetirementReason::FrontendReload,
        _ => return None,
    };
    if fields.next().is_some() {
        return None;
    }
    Some(MainRetirement { launch, navigation, reason })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retirement_reason_is_closed_and_does_not_consume_failure_slots() {
        use super::super::{Diagnostic, Observation};
        use sugarsubstitute_contracts::transcript::OutputStream;
        const LINE: &str =
            "operation=\"main_retirement_requested\" launch=1 navigation=2 reason=FrontendReload";
        assert_eq!(
            parse(LINE.as_bytes()).map(|value| value.reason),
            Some(MainRetirementReason::FrontendReload)
        );
        for bad in [
            LINE.replace("FrontendReload", "PRIVATE"),
            LINE.replace("launch=1", "launch=01"),
            format!("{LINE} cause=PRIVATE"),
        ] {
            assert!(parse(bad.as_bytes()).is_none());
        }
        let mut owner = Diagnostic::new();
        let mut seen = Vec::new();
        for sequence in 1..=32 {
            owner.feed(
                OutputStream::Stderr,
                sequence,
                false,
                false,
                format!("{LINE}\n").as_bytes(),
                |value| seen.push(value),
            );
        }
        assert_eq!(seen.len(), 16);
        owner.feed(
            OutputStream::Stderr,
            33,
            false,
            false,
            b"operation=\"desktop_failure_transition\" boundary=PollFrame kind=Other\n",
            |value| seen.push(value),
        );
        assert!(matches!(seen.last(), Some(Observation::Failure(_))));
    }
}
