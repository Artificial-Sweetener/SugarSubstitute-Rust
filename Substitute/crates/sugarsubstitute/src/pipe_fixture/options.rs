// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate one fixed child invocation without native adoption or secret input.
use super::Result;
use std::{
    error::Error,
    ffi::{OsStr, OsString},
    fmt,
};
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    InheritedLifecycleBootstrap, LifecyclePipeDeadline,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CaseKind {
    Valid,
    WrongCapability,
    ForeignGeneration,
}
impl CaseKind {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::WrongCapability => "wrong-capability",
            Self::ForeignGeneration => "foreign-generation",
        }
    }
    pub(super) const fn exit_code(self) -> u32 {
        match self {
            Self::Valid => 0,
            _ => 37,
        }
    }
}
/// Keep the sole inherited token unadopted until central execution admission.
pub(crate) struct ChildOptions {
    pub(super) section: Option<InheritedLifecycleBootstrap>,
    pub(super) deadline: LifecyclePipeDeadline,
    pub(super) kind: CaseKind,
}
impl ChildOptions {
    /// Accept only section/deadline/frequency/case in this order and no extra input.
    /// Numeric values are positive canonical ASCII; parsing grants no native access.
    pub(crate) fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self> {
        let mut arguments = arguments.into_iter();
        let mut values = [0_u64; 3];
        for (index, flag) in
            ["--section", "--deadline-qpc", "--qpc-frequency"].into_iter().enumerate()
        {
            flag_value(&mut arguments, flag)?;
            values[index] = number(&arguments.next().ok_or(OptionError)?)?;
        }
        flag_value(&mut arguments, "--case")?;
        let value = arguments.next().ok_or(OptionError)?;
        let kind = match value.to_str() {
            Some("valid") => CaseKind::Valid,
            Some("wrong-capability") => CaseKind::WrongCapability,
            Some("foreign-generation") => CaseKind::ForeignGeneration,
            _ => return Err(Box::new(OptionError)),
        };
        if arguments.next().is_some() {
            return Err(Box::new(OptionError));
        }
        Ok(Self {
            section: Some(InheritedLifecycleBootstrap::new(values[0])?),
            deadline: LifecyclePipeDeadline::new(
                i64::try_from(values[1])?,
                i64::try_from(values[2])?,
            )?,
            kind,
        })
    }
}
fn flag_value(arguments: &mut impl Iterator<Item = OsString>, expected: &str) -> Result<()> {
    if arguments.next().as_deref() != Some(OsStr::new(expected)) {
        return Err(Box::new(OptionError));
    }
    Ok(())
}
fn number(value: &OsStr) -> Result<u64> {
    if value.len() > 20 {
        return Err(Box::new(OptionError));
    }
    let value = value.to_str().ok_or(OptionError)?;
    if value.is_empty()
        || value.starts_with('0')
        || !value.bytes().all(|unit| unit.is_ascii_digit())
    {
        return Err(Box::new(OptionError));
    }
    value.parse().map_err(|_| Box::new(OptionError) as Box<dyn Error>)
}
#[derive(Debug)]
struct OptionError;
impl fmt::Display for OptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Lifecycle child requires exact ordered bounded ASCII input")
    }
}
impl Error for OptionError {}
