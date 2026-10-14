// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate the dedicated helper invocation without treating lexical facts as authority.
use std::{
    error::Error,
    ffi::{OsStr, OsString},
    path::PathBuf,
};
use sugarsubstitute_contracts::{
    backend_custody::{BackendCustodyKey, BackendCustodySession},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId},
};
use sugarsubstitute_execution::windows_backend_custody::{
    InheritedLifecycleBootstrap, LifecyclePipeDeadline,
};
/// Retain inert role facts until the genuine inherited bootstrap authenticates them.
pub(super) struct Role {
    pub(super) input: Option<InheritedLifecycleBootstrap>,
    pub(super) original: LifecyclePipeDeadline,
    pub(super) key: BackendCustodyKey,
    pub(super) installation: Option<PathBuf>,
}
impl Role {
    /// Require fixed field order and bounded canonical matching identities.
    pub(super) fn parse(
        arguments: impl IntoIterator<Item = OsString>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut arguments = arguments.into_iter();
        if arguments.next().as_deref() != Some(OsStr::new("--internal-backend-custody")) {
            return Err("Dedicated custody role required".into());
        }
        let input =
            InheritedLifecycleBootstrap::new(field(&mut arguments, "--bootstrap")?.parse()?)?;
        let ticks = field(&mut arguments, "--until-qpc")?.parse()?;
        let frequency = field(&mut arguments, "--qpc-frequency")?.parse()?;
        let namespace = NamespaceId::new(hex(&field(&mut arguments, "--namespace")?)?);
        let epoch = CoordinatorEpoch::new(hex(&field(&mut arguments, "--epoch")?)?)?;
        let session = BackendCustodySession::new(field(&mut arguments, "--session")?.parse()?)?;
        if arguments.next().as_deref() != Some(OsStr::new("--installation")) {
            return Err("Custody installation field missing".into());
        }
        let installation =
            PathBuf::from(arguments.next().ok_or("Custody installation value missing")?);
        if !installation.is_absolute()
            || installation.as_os_str().len() > 32766
            || arguments.next().is_some()
        {
            return Err("Custody role shape rejected".into());
        }
        Ok(Self {
            input: Some(input),
            original: LifecyclePipeDeadline::new(ticks, frequency)?,
            key: BackendCustodyKey::new(CoordinatorKey { namespace, epoch }, session),
            installation: Some(installation),
        })
    }
}
/// Read one bounded ASCII field; capability bytes are never command arguments.
fn field(
    arguments: &mut impl Iterator<Item = OsString>,
    flag: &str,
) -> Result<String, Box<dyn Error>> {
    if arguments.next().as_deref() != Some(OsStr::new(flag)) {
        return Err("Custody field order rejected".into());
    }
    let value = arguments
        .next()
        .ok_or("Custody field absent")?
        .into_string()
        .map_err(|_| "Custody field must be ASCII")?;
    if !value.is_ascii() || value.len() > 64 {
        return Err("Custody field shape rejected".into());
    }
    Ok(value)
}
/// Decode exact lower-case canonical hexadecimal facts, without native effects.
fn hex<const N: usize>(value: &str) -> Result<[u8; N], Box<dyn Error>> {
    if value.len() != N * 2 {
        return Err("Custody identity length rejected".into());
    }
    let digit = |byte| match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err("Custody identity character rejected"),
    };
    let mut result = [0; N];
    for (out, pair) in result.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        *out = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Ok(result)
}
