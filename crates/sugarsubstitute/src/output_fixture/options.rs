// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Parse fixed roles and nonsecret bootstrap facts before any native effects.
use super::Result;
use std::ffi::{OsStr, OsString};
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    InheritedLifecycleBootstrap, LifecyclePipeDeadline,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Role {
    Corpus,
    Flood,
    Idle,
    Forward,
    Leaf,
    Prelude,
}
impl Role {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Corpus => "corpus",
            Self::Flood => "flood",
            Self::Idle => "idle",
            Self::Forward => "forward",
            Self::Leaf => "leaf",
            Self::Prelude => "prelude",
        }
    }
    pub(super) const fn authenticated(self) -> bool {
        matches!(self, Self::Flood | Self::Idle | Self::Forward)
    }
}
/// One explicit role; parsing a section token never adopts a native handle.
pub(crate) struct ChildOptions {
    pub(super) role: Role,
    pub(super) section: Option<InheritedLifecycleBootstrap>,
    pub(super) deadline: Option<LifecyclePipeDeadline>,
}
impl ChildOptions {
    /// Reject reordered, extra, lossy or noncanonical fields without native calls.
    pub(crate) fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self> {
        let mut args = arguments.into_iter();
        flag(&mut args, "--case")?;
        let role = match args.next().as_deref().and_then(OsStr::to_str) {
            Some("corpus") => Role::Corpus,
            Some("flood") => Role::Flood,
            Some("idle") => Role::Idle,
            Some("forward") => Role::Forward,
            Some("leaf") => Role::Leaf,
            _ => return Err("Output child role is invalid".into()),
        };
        let (section, deadline) = if role.authenticated() {
            let mut values = [0_u64; 3];
            for (i, name) in ["--section", "--deadline-qpc", "--qpc-frequency"].iter().enumerate() {
                flag(&mut args, name)?;
                let value = args.next().ok_or("Output bootstrap value missing")?;
                let text = value.to_str().ok_or("Output bootstrap is not ASCII")?;
                if text.len() > 20
                    || text.is_empty()
                    || text.starts_with('0')
                    || !text.bytes().all(|b| b.is_ascii_digit())
                {
                    return Err("Output bootstrap must be positive canonical ASCII".into());
                }
                values[i] = text.parse()?;
            }
            (
                Some(InheritedLifecycleBootstrap::new(values[0])?),
                Some(LifecyclePipeDeadline::new(
                    i64::try_from(values[1])?,
                    i64::try_from(values[2])?,
                )?),
            )
        } else {
            (None, None)
        };
        if args.next().is_some() {
            return Err("Output child rejects extra fields".into());
        }
        Ok(Self { role, section, deadline })
    }
}
fn flag(args: &mut impl Iterator<Item = OsString>, name: &str) -> Result<()> {
    if args.next().as_deref() != Some(OsStr::new(name)) {
        return Err("Output child flag/order differs".into());
    }
    Ok(())
}
