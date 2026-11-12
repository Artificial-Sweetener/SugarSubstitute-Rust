// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode bounded qualification inputs without native ownership or authentication.
use super::Result;
use std::{
    ffi::{OsStr, OsString},
    path::{Component, PathBuf},
};
use sugarsubstitute_contracts::lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId};
use sugarsubstitute_execution::{
    windows::NativeCorrelation,
    windows_fixture::{FixtureDeadline, InheritedFixtureEvents},
};

/// Own one newly created campaign workspace and its final diagnostic destination.
pub(crate) struct CampaignOptions {
    /// Fresh artifact directory; an existing directory is rejected before setup.
    pub(crate) workspace: PathBuf,
    /// Parent receipt written after native and execution teardown.
    pub(crate) receipt: PathBuf,
}
impl CampaignOptions {
    /// Require exactly two ordered absolute D-drive artifact paths.
    pub(crate) fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self> {
        let mut arguments = arguments.into_iter();
        let workspace = path(&mut arguments, "--workspace")?;
        let receipt = path(&mut arguments, "--receipt")?;
        exhausted(&mut arguments)?;
        Ok(Self { workspace, receipt })
    }
}

/// Fixed same-executable evidence roles; none authenticates an IPC peer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    CoordinatorContender,
    BackendContender,
    BackendSuccess,
    ActiveHolder,
    ActivePredecessor,
}
impl Role {
    /// Use a bounded wire spelling shared by options and diagnostic receipts.
    pub(crate) const fn wire(self) -> &'static str {
        match self {
            Self::CoordinatorContender => "coordinator-contender",
            Self::BackendContender => "backend-contender",
            Self::BackendSuccess => "backend-success",
            Self::ActiveHolder => "active-holder",
            Self::ActivePredecessor => "active-predecessor",
        }
    }
    /// Identify the exact native failure required before acknowledging denial.
    pub(crate) const fn expected_failure(self) -> Option<&'static str> {
        match self {
            Self::CoordinatorContender | Self::BackendContender => Some("claim_exclusion"),
            Self::ActivePredecessor => Some("unresolved_claim_predecessor"),
            Self::BackendSuccess | Self::ActiveHolder => None,
        }
    }
}

/// Keep exclusively inherited event tokens until one admitted child bind.
pub(crate) struct ChildOptions {
    pub(crate) role: Role,
    pub(crate) installation: PathBuf,
    pub(crate) backend: PathBuf,
    pub(crate) receipt: PathBuf,
    /// Parent's actual matching key is only a partial-initialization cleanup context.
    pub(crate) parent: CoordinatorKey,
    pub(crate) events: Option<InheritedFixtureEvents>,
    pub(crate) deadline: FixtureDeadline,
    pub(crate) correlation: NativeCorrelation,
}
impl ChildOptions {
    /// Decode fixed ordered fields; constructor validation grants no native authority.
    pub(crate) fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self> {
        let mut arguments = arguments.into_iter();
        flag(&mut arguments, "--role")?;
        let role = arguments
            .next()
            .and_then(|v| match v.to_str() {
                Some("coordinator-contender") => Some(Role::CoordinatorContender),
                Some("backend-contender") => Some(Role::BackendContender),
                Some("backend-success") => Some(Role::BackendSuccess),
                Some("active-holder") => Some(Role::ActiveHolder),
                Some("active-predecessor") => Some(Role::ActivePredecessor),
                _ => None,
            })
            .ok_or("Claim child has an invalid evidence role")?;
        let installation = path(&mut arguments, "--installation")?;
        let backend = path(&mut arguments, "--backend")?;
        let receipt = path(&mut arguments, "--receipt")?;
        flag(&mut arguments, "--namespace")?;
        let namespace = hex::<32>(&arguments.next().ok_or("Claim child namespace missing")?)?;
        flag(&mut arguments, "--epoch")?;
        let epoch = hex::<16>(&arguments.next().ok_or("Claim child epoch missing")?)?;
        let parent = CoordinatorKey {
            namespace: NamespaceId::new(namespace),
            epoch: CoordinatorEpoch::new(epoch)?,
        };
        let mut values = [0; 7];
        for (index, name) in [
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
            flag(&mut arguments, name)?;
            values[index] =
                positive(&arguments.next().ok_or("Claim child numeric input missing")?)?;
        }
        exhausted(&mut arguments)?;
        Ok(Self {
            role,
            installation,
            backend,
            receipt,
            parent,
            events: Some(InheritedFixtureEvents::new(values[0], values[1], values[2], values[3])?),
            deadline: FixtureDeadline::new(values[4], values[5])?,
            correlation: NativeCorrelation::new(values[6])?,
        })
    }
}
fn flag(arguments: &mut impl Iterator<Item = OsString>, expected: &str) -> Result<()> {
    if arguments.next().as_deref() != Some(OsStr::new(expected)) {
        return Err("Claim fixture requires exact ordered inputs".into());
    }
    Ok(())
}
fn exhausted(arguments: &mut impl Iterator<Item = OsString>) -> Result<()> {
    if arguments.next().is_some() {
        return Err("Claim fixture rejects trailing input".into());
    }
    Ok(())
}
fn path(arguments: &mut impl Iterator<Item = OsString>, name: &str) -> Result<PathBuf> {
    flag(arguments, name)?;
    let value = arguments.next().ok_or("Claim fixture path missing")?;
    if value.len() > 8192 {
        return Err("Claim fixture path bound exceeded".into());
    }
    let path = PathBuf::from(value);
    if !path.is_absolute()
        || path.components().any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        || !path.starts_with(r"D:\SugarSubstitute-Rust\artifacts\m1")
    {
        return Err("Claim fixture requires a bounded absolute M1 artifact path on D".into());
    }
    Ok(path)
}
fn positive(value: &OsStr) -> Result<u64> {
    if value.len() > 20 {
        return Err("Claim child numeric input is invalid".into());
    }
    let text = value.to_str().ok_or("Claim child numeric input is invalid")?;
    if text.is_empty() || text.starts_with('0') || !text.bytes().all(|v| v.is_ascii_digit()) {
        return Err("Claim child numeric input is invalid".into());
    }
    Ok(text.parse()?)
}
fn hex<const N: usize>(value: &OsStr) -> Result<[u8; N]> {
    if value.len() != N * 2 {
        return Err("Claim child matching key is invalid".into());
    }
    let text = value.to_str().ok_or("Claim child matching key is invalid")?;
    let mut result = [0; N];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let nibble = |v| match v {
            b'0'..=b'9' => Ok(v - b'0'),
            b'a'..=b'f' => Ok(v - b'a' + 10),
            _ => Err("Claim child matching key is invalid"),
        };
        result[index] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Ok(result)
}
/// Render matching identities without paths, handle tokens or authority claims.
pub(super) fn hex_text(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|v| [DIGITS[(v >> 4) as usize] as char, DIGITS[(v & 15) as usize] as char])
        .collect()
}

#[cfg(test)]
mod tests;
