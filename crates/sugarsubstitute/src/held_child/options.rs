// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode one fixed ASCII child invocation without accepting native ownership.
use super::Result;
use std::{
    error::Error,
    ffi::{OsStr, OsString},
    fmt,
};
use sugarsubstitute_execution::{
    windows::NativeCorrelation,
    windows_fixture::{FixtureDeadline, InheritedFixtureEvents},
};

/// Retain validated role-order input until one successful admitted bind.
/// No native operation or handle closure occurs during parsing or unbound Drop.
pub(crate) struct ChildOptions {
    pub(super) events: Option<InheritedFixtureEvents>,
    pub(super) deadline: FixtureDeadline,
    pub(super) correlation: NativeCorrelation,
    pub(super) exit_code: i32,
}
impl ChildOptions {
    /// Require exactly eight ordered flag/value pairs and canonical ASCII numbers.
    /// Event tokens, QPC/frequency and correlation are positive; only exit0/37/259
    /// is supported. Embedded signs, leading zeroes, overflow, extra input and
    /// non-Unicode/non-ASCII tokens reject with redacted diagnostics before effects.
    pub(crate) fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self> {
        let mut arguments = arguments.into_iter();
        let mut values = [0; 7];
        for (index, flag) in [
            "--ready",
            "--release",
            "--challenge",
            "--response",
            "--deadline-qpc",
            "--qpc-frequency",
            "--correlation",
        ]
        .into_iter()
        .enumerate()
        {
            require_flag(&mut arguments, flag)?;
            let value = arguments.next().ok_or(OptionError)?;
            values[index] = positive_decimal(&value)?;
        }
        require_flag(&mut arguments, "--exit-code")?;
        let exit = arguments.next().ok_or(OptionError)?;
        if exit.len() > 3 {
            return Err(Box::new(OptionError));
        }
        let exit_code = match exit.to_str() {
            Some("0") => 0,
            Some("37") => 37,
            Some("259") => 259,
            _ => return Err(Box::new(OptionError)),
        };
        if arguments.next().is_some() {
            return Err(Box::new(OptionError));
        }
        Ok(Self {
            events: Some(InheritedFixtureEvents::new(values[0], values[1], values[2], values[3])?),
            deadline: FixtureDeadline::new(values[4], values[5])?,
            correlation: NativeCorrelation::new(values[6])?,
            exit_code,
        })
    }
}
fn require_flag(arguments: &mut impl Iterator<Item = OsString>, expected: &str) -> Result<()> {
    if arguments.next().as_deref() != Some(OsStr::new(expected)) {
        return Err(Box::new(OptionError));
    }
    Ok(())
}
fn positive_decimal(value: &OsStr) -> Result<u64> {
    if value.len() > 20 {
        return Err(Box::new(OptionError));
    }
    let value = value.to_str().ok_or(OptionError)?;
    if value.is_empty()
        || value.len() > 20
        || value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Box::new(OptionError));
    }
    value.parse::<u64>().map_err(|_| Box::new(OptionError) as Box<dyn Error>)
}
#[derive(Debug)]
struct OptionError;
impl fmt::Display for OptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Held child invocation requires exact ordered bounded ASCII inputs")
    }
}
impl Error for OptionError {}
