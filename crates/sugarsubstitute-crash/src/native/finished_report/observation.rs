// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate native result provenance and retain the genuine first and secondary causes.
use super::{abi::Output, control::ControlCause};
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Origin {
    Native,
    Control,
    Qpc,
}
impl Origin {
    fn checked(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::Native),
            2 => Some(Self::Control),
            3 => Some(Self::Qpc),
            _ => None,
        }
    }
}
/// External native result is matching data; this error grants no disposition or close proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct InvalidResult;
impl fmt::Display for InvalidResult {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Invalid finished-report result")
    }
}
impl Error for InvalidResult {}

/// Preserve actual code and its reported origin without comparing code values for precedence.
#[derive(Debug)]
pub(super) enum Failure {
    Native { origin: Origin, code: u32 },
    Control(ControlCause),
    Invalid(InvalidResult),
}
impl fmt::Display for Failure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native { origin, code } => write!(out, "Finished-report {origin:?} error {code}"),
            Self::Control(source) => fmt::Display::fmt(source, out),
            Self::Invalid(source) => fmt::Display::fmt(source, out),
        }
    }
}
impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Control(source) => Some(source),
            Self::Invalid(source) => Some(source),
            _ => None,
        }
    }
}
/// Inline cells retain the original cause, bounded independent causes and omissions.
/// This storage must remain with the token's separately admitted Rust destination.
pub(super) struct Failures {
    pub first: Option<Failure>,
    pub secondary: [Option<Failure>; 8],
    pub omitted: u64,
    primary: Option<(Origin, u32)>,
    codes: [Option<(Origin, u32)>; 8],
    native_omitted: u64,
}
impl Failures {
    pub(super) const fn new() -> Self {
        Self {
            first: None,
            secondary: [const { None }; 8],
            omitted: 0,
            primary: None,
            codes: [None; 8],
            native_omitted: 0,
        }
    }
    pub(super) fn retain(&mut self, cause: Failure) {
        if self.first.is_none() {
            self.first = Some(cause)
        } else if let Some(cell) = self.secondary.iter_mut().find(|cell| cell.is_none()) {
            *cell = Some(cause)
        } else {
            self.omitted = self.omitted.saturating_add(1)
        }
    }
    /// Adopt result codes in their actual latching order and consume one original control cause.
    /// No native token may be discarded merely because validation fails here.
    pub(super) fn observe(
        &mut self,
        result: &Output,
        control: Option<ControlCause>,
    ) -> Result<(), InvalidResult> {
        let mut control = control;
        let observed = self.observe_checked(result, &mut control);
        if observed.is_err() {
            self.retain(Failure::Invalid(InvalidResult))
        }
        if let Some(control) = control {
            self.retain(Failure::Control(control));
        }
        observed
    }
    fn observe_checked(
        &mut self,
        result: &Output,
        control: &mut Option<ControlCause>,
    ) -> Result<(), InvalidResult> {
        validate(result)?;
        if let Some(origin) = Origin::checked(result.primary_origin) {
            let actual = (origin, result.primary_error);
            if let Some(previous) = self.primary {
                if previous != actual {
                    return Err(InvalidResult);
                }
            } else {
                self.retained_code(origin, result.primary_error, control)?;
                self.primary = Some(actual);
            }
        }
        for index in 0..result.cleanup_count as usize {
            let origin = Origin::checked((result.cleanup_origin_bits >> (index * 2)) & 3)
                .ok_or(InvalidResult)?;
            let actual = (origin, result.cleanup_codes[index]);
            if let Some(previous) = self.codes[index] {
                if previous != actual {
                    return Err(InvalidResult);
                }
            } else {
                self.retained_code(origin, result.cleanup_codes[index], control)?;
                self.codes[index] = Some(actual);
            }
        }
        if result.omitted_cleanup < self.native_omitted {
            return Err(InvalidResult);
        }
        self.omitted = self.omitted.saturating_add(result.omitted_cleanup - self.native_omitted);
        self.native_omitted = result.omitted_cleanup;
        Ok(())
    }
    fn retained_code(
        &mut self,
        origin: Origin,
        code: u32,
        control: &mut Option<ControlCause>,
    ) -> Result<(), InvalidResult> {
        if origin == Origin::Control {
            if let Some(source) = control.take() {
                let matches = source.status().1 == code;
                self.retain(Failure::Control(source));
                if !matches {
                    return Err(InvalidResult);
                }
            } else if self.primary != Some((Origin::Control, code))
                && !self.codes.contains(&Some((Origin::Control, code)))
            {
                return Err(InvalidResult);
            }
        } else {
            self.retain(Failure::Native { origin, code })
        }
        Ok(())
    }
}
fn validate(result: &Output) -> Result<(), InvalidResult> {
    if result.version != 1
        || result.status > 7
        || result.cleanup_count > 8
        || result.retired > 1
        || result.facts_valid > 1
        || result.handles > 11
        || result.locks > 1
        || result.cleanup_origin_bits >> 16 != 0
        || (result.primary_error == 0) != (result.primary_origin == 0)
        || (result.primary_error != 0 && Origin::checked(result.primary_origin).is_none())
        || (result.retired == 1 && (result.lease != 0 || result.handles != 0 || result.locks != 0))
    {
        return Err(InvalidResult);
    }
    for index in 0..8 {
        let origin = (result.cleanup_origin_bits >> (index * 2)) & 3;
        if index < result.cleanup_count as usize {
            if result.cleanup_codes[index] == 0 || Origin::checked(origin).is_none() {
                return Err(InvalidResult);
            }
        } else if result.cleanup_codes[index] != 0 || origin != 0 {
            return Err(InvalidResult);
        }
    }
    Ok(())
}
#[cfg(test)]
#[path = "observation/tests.rs"]
mod tests;
