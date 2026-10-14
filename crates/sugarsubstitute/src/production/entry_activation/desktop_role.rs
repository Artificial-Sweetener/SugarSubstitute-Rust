// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the sole native desktop-role argument grammar shared by parent and child.
#[cfg(test)]
#[path = "desktop_role_tests.rs"]
mod tests;
use std::{
    error::Error,
    ffi::{OsStr, OsString},
    os::windows::io::AsRawHandle,
};
use sugarsubstitute_contracts::{
    execution::Generation,
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    InheritedLifecycleBootstrap, LifecyclePipeBootstrap, LifecyclePipeDeadline,
};

/// Lexical child input; the actual captured transport authenticates its inherited section.
pub(crate) struct DesktopRole {
    pub(crate) qualification: super::super::qualification::QualificationIntent,
    pub(crate) bootstrap: InheritedLifecycleBootstrap,
    pub(crate) original: LifecyclePipeDeadline,
    pub(crate) run: RunKey,
    pub(crate) launch: LaunchId,
    pub(crate) navigation: Generation,
}
impl DesktopRole {
    /// Validate an exact dedicated role without native effects or environment lookup.
    pub(crate) fn parse(
        arguments: impl IntoIterator<Item = OsString>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut arguments = arguments.into_iter();
        if arguments.next().as_deref() != Some(OsStr::new("--internal-production-desktop")) {
            return Err("Dedicated desktop role required".into());
        }
        let bootstrap = InheritedLifecycleBootstrap::new(number(&mut arguments, "--bootstrap")?)?;
        let ticks = integer(&mut arguments, "--until-qpc")?;
        let frequency = integer(&mut arguments, "--qpc-frequency")?;
        let namespace = NamespaceId::new(hex::<32>(&value(&mut arguments, "--namespace")?)?);
        let epoch = CoordinatorEpoch::new(hex::<16>(&value(&mut arguments, "--epoch")?)?)?;
        let instance = InstanceId::new(number(&mut arguments, "--instance")?)?;
        let generation = RunGeneration::new(number(&mut arguments, "--run-generation")?)?;
        let launch = LaunchId::new(number(&mut arguments, "--launch")?)?;
        let navigation = Generation::new(number(&mut arguments, "--navigation")?)?;
        let qualification =
            super::super::qualification::QualificationIntent::child(&mut arguments)?;
        Ok(Self {
            qualification,
            bootstrap,
            original: LifecyclePipeDeadline::new(ticks, frequency)?,
            run: RunKey { owner: CoordinatorKey { namespace, epoch }, instance, generation },
            launch,
            navigation,
        })
    }
}
/// Encode only facts from the actual pinned bootstrap and the retained parent run.
/// This runs under the existing projected entry charge; no secret capability is copied.
pub(super) fn arguments(
    bootstrap: &LifecyclePipeBootstrap,
    run: RunKey,
    launch: LaunchId,
    navigation: Generation,
    qualification: &super::super::qualification::QualificationIntent,
) -> Result<Vec<Vec<u16>>, Box<dyn Error + Send + Sync>> {
    if bootstrap.run() != Some(run) {
        return Err("Desktop bootstrap run mismatch".into());
    }
    let original = bootstrap.deadline().ok_or("Desktop bootstrap deadline unavailable")?;
    let inherited = bootstrap.borrow_inheritance();
    let token = inherited.section.as_raw_handle() as usize;
    let mut output = Vec::new();
    output.try_reserve_exact(19)?;
    for value in [
        "--internal-production-desktop".to_owned(),
        "--bootstrap".to_owned(),
        token.to_string(),
        "--until-qpc".to_owned(),
        original.ticks().to_string(),
        "--qpc-frequency".to_owned(),
        original.frequency().to_string(),
        "--namespace".to_owned(),
        encode_hex(run.owner.namespace.bytes()),
        "--epoch".to_owned(),
        encode_hex(run.owner.epoch.bytes()),
        "--instance".to_owned(),
        run.instance.get().to_string(),
        "--run-generation".to_owned(),
        run.generation.get().to_string(),
        "--launch".to_owned(),
        launch.get().to_string(),
        "--navigation".to_owned(),
        navigation.get().to_string(),
    ] {
        let mut units = Vec::new();
        units.try_reserve_exact(value.len())?;
        units.extend(value.encode_utf16());
        output.push(units);
    }
    qualification.append(&mut output)?;
    Ok(output)
}
fn value(
    arguments: &mut impl Iterator<Item = OsString>,
    flag: &str,
) -> Result<String, Box<dyn Error>> {
    if arguments.next().as_deref() != Some(OsStr::new(flag)) {
        return Err("Desktop role field order mismatch".into());
    }
    arguments
        .next()
        .ok_or("Desktop role field missing")?
        .into_string()
        .map_err(|_| "Desktop role field must be ASCII".into())
}
fn number(
    arguments: &mut impl Iterator<Item = OsString>,
    flag: &str,
) -> Result<u64, Box<dyn Error>> {
    Ok(value(arguments, flag)?.parse()?)
}
fn integer(
    arguments: &mut impl Iterator<Item = OsString>,
    flag: &str,
) -> Result<i64, Box<dyn Error>> {
    Ok(value(arguments, flag)?.parse()?)
}
fn hex<const N: usize>(value: &str) -> Result<[u8; N], Box<dyn Error>> {
    if value.len() != N * 2 || !value.is_ascii() {
        return Err("Desktop role identity shape".into());
    }
    let mut result = [0; N];
    for (destination, pair) in result.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        let high = digit(pair[0])?;
        let low = digit(pair[1])?;
        *destination = (high << 4) | low;
    }
    Ok(result)
}
fn digit(value: u8) -> Result<u8, Box<dyn Error>> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err("Desktop role identity encoding".into()),
    }
}
fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for value in bytes {
        result.push(HEX[usize::from(value >> 4)] as char);
        result.push(HEX[usize::from(value & 15)] as char);
    }
    result
}
