// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep one carrier invocation grammar for the genuine Root producer and inert child parser.
use std::{
    error::Error,
    ffi::{OsStr, OsString},
    os::windows::io::AsRawHandle,
};
use sugarsubstitute_execution::windows_recovery_supervisor::{
    InheritedRecoveryBootstrap, LifecyclePipeDeadline, RecoveryCarrierBootstrap,
};
const PREFIX: &str = "--internal-recovery-supervisor";
const FIELDS: [&str; 5] =
    ["--bootstrap", "--desktop-bootstrap", "--job", "--until-qpc", "--qpc-frequency"];
/// Inert numeric slots are adopted only by their authenticated native owners.
pub(super) struct Role {
    pub(super) input: Option<InheritedRecoveryBootstrap>,
    pub(super) job: Option<sugarsubstitute_supervision::InheritedRecoveryJobInput>,
    pub(super) original: LifecyclePipeDeadline,
}
/// Match only this dedicated role without consuming or changing ordinary invocation arguments.
pub(in crate::production) fn matches_prefix(value: &OsStr) -> bool {
    value == OsStr::new(PREFIX)
}
impl Role {
    /// Require canonical fixed-order numeric arguments and reject every trailing field.
    /// No capability, path, environment or startup bytes belong to this invocation.
    pub(super) fn parse(
        arguments: impl IntoIterator<Item = OsString>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut arguments = arguments.into_iter();
        if arguments.next().as_deref().is_none_or(|value| !matches_prefix(value)) {
            return Err("Dedicated recovery supervisor role required".into());
        }
        let mut values = [0; FIELDS.len()];
        for (value, flag) in values.iter_mut().zip(FIELDS) {
            *value = number(&mut arguments, flag)?;
        }
        if arguments.next().is_some() {
            return Err("Recovery supervisor trailing arguments rejected".into());
        }
        let [recovery, desktop, job, ticks, frequency] = values;
        validate_slots(recovery, desktop, job)?;
        Ok(Self {
            input: Some(InheritedRecoveryBootstrap::new(recovery, desktop)?),
            job: Some(sugarsubstitute_supervision::InheritedRecoveryJobInput::from_token(
                usize::try_from(job)?,
            )?),
            original: LifecyclePipeDeadline::new(i64::try_from(ticks)?, i64::try_from(frequency)?)?,
        })
    }
}
/// Encode the actual two retained readonly pins and published profile Job slot once.
/// The Root's admitted projection/creation owner funds these bounded argument copies.
/// The borrowed sections stay retained through actual native attribute deletion;
/// numeric Job metadata alone never constructs a profile or recovery capability.
/// # Errors
/// Rejects missing original counters, slot alias/pseudo values, capacity or allocation failure.
pub(in crate::production) fn arguments(
    bootstrap: &RecoveryCarrierBootstrap,
    published_job_token: usize,
) -> Result<Vec<Vec<u16>>, Box<dyn Error + Send + Sync>> {
    let original = bootstrap.deadline().ok_or("Original recovery carrier counter absent")?;
    let pins = bootstrap.borrow_inheritance();
    let recovery = pins[0].section.as_raw_handle() as usize;
    let desktop = pins[1].section.as_raw_handle() as usize;
    let slots = [recovery as u64, desktop as u64, published_job_token as u64];
    validate_slots(slots[0], slots[1], slots[2])?;
    encode([
        slots[0],
        slots[1],
        slots[2],
        u64::try_from(original.ticks())?,
        u64::try_from(original.frequency())?,
    ])
}
/// Keep encoder/parser field ordering authoritative in the same fixed registry.
fn encode(values: [u64; 5]) -> Result<Vec<Vec<u16>>, Box<dyn Error + Send + Sync>> {
    let mut output = Vec::new();
    output.try_reserve_exact(1 + FIELDS.len() * 2)?;
    push_text(&mut output, PREFIX)?;
    for (flag, value) in FIELDS.into_iter().zip(values) {
        push_text(&mut output, flag)?;
        let mut text = String::new();
        text.try_reserve_exact(20)?;
        use std::fmt::Write;
        write!(&mut text, "{value}")?;
        push_text(&mut output, &text)?;
    }
    Ok(output)
}
/// Allocate one bounded ASCII argument inside the caller's original projection charge.
fn push_text(
    output: &mut Vec<Vec<u16>>,
    text: &str,
) -> Result<(), std::collections::TryReserveError> {
    let mut units = Vec::new();
    units.try_reserve_exact(text.len())?;
    units.extend(text.encode_utf16());
    output.push(units);
    Ok(())
}
/// Validate slot facts before either parser or producer can publish a conflicting role shape.
fn validate_slots(recovery: u64, desktop: u64, job: u64) -> Result<(), &'static str> {
    let limit = (usize::MAX - 15) as u64;
    if [recovery, desktop, job].into_iter().any(|value| value == 0 || value >= limit)
        || recovery == desktop
        || recovery == job
        || desktop == job
    {
        return Err("Recovery inherited native slots rejected");
    }
    Ok(())
}
/// Limit lexical parsing to canonical positive decimal before native admission.
fn number(
    arguments: &mut impl Iterator<Item = OsString>,
    flag: &str,
) -> Result<u64, Box<dyn Error>> {
    if arguments.next().as_deref() != Some(OsStr::new(flag)) {
        return Err("Recovery supervisor field order rejected".into());
    }
    let value = arguments
        .next()
        .ok_or("Recovery supervisor field missing")?
        .into_string()
        .map_err(|_| "Recovery supervisor field must be ASCII")?;
    if value.is_empty()
        || value.len() > 20
        || value.as_bytes()[0] == b'0'
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("Recovery supervisor numeric field rejected".into());
    }
    Ok(value.parse()?)
}
#[cfg(test)]
#[path = "role_tests.rs"]
mod tests;
