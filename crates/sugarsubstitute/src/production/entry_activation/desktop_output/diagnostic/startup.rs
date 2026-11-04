// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode closed startup failure enrichment inside the existing first-failure reservation.
//!
//! The desktop owns every token of this frame; decoding uses its parsers, so a fact the
//! desktop can emit is never dropped here for want of a second vocabulary.

use sugarsubstitute_desktop::{
    StartupCaptureState as Capture, StartupCauseCode as Code, StartupCloseStage as Stage,
    StartupObservationKind as ObservationKind,
};

/// Inert actual observation and first-close facts; Run belongs to the authenticated output owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct StartupFailureDetail {
    pub(in super::super) observation: ObservationKind,
    pub(in super::super) stage: Stage,
    pub(in super::super) capture: Capture,
    pub(in super::super) code: Code,
}

/// Require complete authored fields, canonical numeric causes and no raw extensions.
pub(super) fn parse(
    mut fields: std::str::SplitAsciiWhitespace<'_>,
) -> Option<StartupFailureDetail> {
    let observation = ObservationKind::parse(fields.next()?.strip_prefix("observation=")?)?;
    let stage = Stage::parse(fields.next()?.strip_prefix("close_stage=")?)?;
    let capture = Capture::parse(fields.next()?.strip_prefix("capture_snapshot=")?)?;
    let code = Code::parse(fields.next()?.strip_prefix("code=")?)?;
    if fields.next().is_some() {
        return None;
    }
    Some(StartupFailureDetail { observation, stage, capture, code })
}

#[cfg(test)]
#[path = "startup_tests.rs"]
mod tests;
