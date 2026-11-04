// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check that each child delivery category round-trips through its relay token only.
use super::super::vocabulary::parse_terminal_kind;
use super::*;
use crate::StartupCauseCode;
use sugarsubstitute_contracts::execution::{ExecutionError, Operation, TimeError};

#[test]
fn every_child_category_parses_its_own_debug_token_and_refuses_variants() {
    for &cause in StartupPipeCause::ALL {
        assert_eq!(StartupPipeCause::parse(&format!("{cause:?}")), Some(cause));
    }
    for &cause in StartupChallengeCause::ALL {
        assert_eq!(StartupChallengeCause::parse(&format!("{cause:?}")), Some(cause));
    }
    for &cause in StartupTimeCause::ALL {
        assert_eq!(StartupTimeCause::parse(&format!("{cause:?}")), Some(cause));
    }
    for kind in [
        TerminalKind::Success,
        TerminalKind::Failure,
        TerminalKind::Panicked,
        TerminalKind::Cancelled,
        TerminalKind::TimedOut,
        TerminalKind::Coalesced,
    ] {
        assert_eq!(parse_terminal_kind(&format!("{kind:?}")), Some(kind));
        let terminal = StartupOutputCause::Terminal(kind);
        assert_eq!(StartupOutputCause::parse(&format!("{terminal:?}")), Some(terminal));
    }
    for cause in [StartupOutputCause::Deadline, StartupOutputCause::Order] {
        assert_eq!(StartupOutputCause::parse(&format!("{cause:?}")), Some(cause));
    }
    for bad in ["busy", "Busy ", "Busy(1)", "", "Terminal(Unknown)", "Terminal(TimedOut", "PRIVATE"]
    {
        assert_eq!(StartupPipeCause::parse(bad), None);
        assert_eq!(StartupChallengeCause::parse(bad), None);
        assert_eq!(StartupOutputCause::parse(bad), None);
        assert_eq!(StartupTimeCause::parse(bad), None);
    }
}

#[test]
fn relay_tokens_name_each_category_and_nested_terminal_kind() {
    for (code, token) in [
        (StartupCauseCode::Pipe(StartupPipeCause::DeadlineExpired), "Pipe(DeadlineExpired)"),
        (StartupCauseCode::Challenge(StartupChallengeCause::Busy), "Challenge(Busy)"),
        (StartupCauseCode::Output(StartupOutputCause::Deadline), "Output(Deadline)"),
        (
            StartupCauseCode::Output(StartupOutputCause::Terminal(TerminalKind::TimedOut)),
            "Output(Terminal(TimedOut))",
        ),
        (StartupCauseCode::Time(StartupTimeCause::Regression), "Time(Regression)"),
        (StartupCauseCode::Execution, "Execution"),
        (StartupCauseCode::Protocol, "Protocol"),
        (StartupCauseCode::MainTerminal(TerminalKind::Failure), "MainTerminal(Failure)"),
    ] {
        assert_eq!(code.to_string(), token);
    }
}

#[test]
fn direct_projection_names_clock_and_execution_owner_failures() {
    assert_eq!(
        StartupCauseCode::direct(&TimeError::Overflow),
        StartupCauseCode::Time(StartupTimeCause::Overflow)
    );
    assert_eq!(
        StartupCauseCode::direct(&TimeError::Regression),
        StartupCauseCode::Time(StartupTimeCause::Regression)
    );
    let execution =
        ExecutionError::new(Operation::Infrastructure, Box::new(std::io::Error::other("PRIVATE")));
    assert_eq!(StartupCauseCode::direct(&execution), StartupCauseCode::Execution);
}
