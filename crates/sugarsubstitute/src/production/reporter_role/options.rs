// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Share the sole Reporter invocation grammar without accepting storage paths or privileges.
use std::{
    error::Error,
    ffi::{OsStr, OsString},
    os::windows::io::AsRawHandle,
};
use sugarsubstitute_contracts::configuration::LocaleId;
use sugarsubstitute_contracts::incident::DiagnosticRole;
use sugarsubstitute_execution::windows_diagnostic_pipe::{
    DiagnosticPipeBootstrap, InheritedLifecycleBootstrap, LifecyclePipeDeadline,
};

/// Lexical data is inert until the actual inherited section verifies parent and role.
pub(in crate::production) struct ReporterRole {
    pub(in crate::production) bootstrap: InheritedLifecycleBootstrap,
    pub(in crate::production) original: LifecyclePipeDeadline,
    pub(in crate::production) locale: LocaleId,
}
impl ReporterRole {
    /// Require one exact dedicated role and the original counter, before native adoption.
    pub(in crate::production) fn parse(
        arguments: impl IntoIterator<Item = OsString>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut arguments = arguments.into_iter();
        if arguments.next().as_deref() != Some(OsStr::new("--internal-production-reporter")) {
            return Err("Dedicated Reporter role required".into());
        }
        let token = value(&mut arguments, "--bootstrap")?.parse()?;
        let ticks = value(&mut arguments, "--until-qpc")?.parse()?;
        let frequency = value(&mut arguments, "--qpc-frequency")?.parse()?;
        let locale = LocaleId::new(&value(&mut arguments, "--locale")?)?;
        if arguments.next().is_some() {
            return Err("Unexpected Reporter role arguments".into());
        }
        Ok(Self {
            bootstrap: InheritedLifecycleBootstrap::new(token)?,
            original: LifecyclePipeDeadline::new(ticks, frequency)?,
            locale,
        })
    }
}
fn value(
    arguments: &mut impl Iterator<Item = OsString>,
    flag: &str,
) -> Result<String, Box<dyn Error>> {
    if arguments.next().as_deref() != Some(OsStr::new(flag)) {
        return Err("Reporter role field order mismatch".into());
    }
    let value = arguments
        .next()
        .ok_or("Reporter role field missing")?
        .into_string()
        .map_err(|_| "Reporter role field must be ASCII")?;
    if !value.is_ascii() {
        return Err("Reporter role field must be ASCII".into());
    }
    Ok(value)
}
/// Encode only the original pinned inherited section and its actual original counter.
/// The composing owner runs this under its existing bounded launch-input reservation.
pub(in crate::production) fn arguments(
    bootstrap: &DiagnosticPipeBootstrap,
    locale: LocaleId,
) -> Result<Vec<Vec<u16>>, Box<dyn Error + Send + Sync>> {
    if bootstrap.key().role() != DiagnosticRole::Reporter {
        return Err("Reporter bootstrap role mismatch".into());
    }
    let original = bootstrap.deadline().ok_or("Reporter bootstrap deadline unavailable")?;
    let inheritance = bootstrap.borrow_inheritance();
    let token = inheritance.section.as_raw_handle() as usize;
    let mut output = Vec::new();
    output.try_reserve_exact(9)?;
    for value in [
        "--internal-production-reporter".to_owned(),
        "--bootstrap".to_owned(),
        token.to_string(),
        "--until-qpc".to_owned(),
        original.ticks().to_string(),
        "--qpc-frequency".to_owned(),
        original.frequency().to_string(),
        "--locale".to_owned(),
        locale.as_str().to_owned(),
    ] {
        let mut units = Vec::new();
        units.try_reserve_exact(value.len())?;
        units.extend(value.encode_utf16());
        output.push(units);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn values(extra: &[&str]) -> Vec<OsString> {
        [
            "--internal-production-reporter",
            "--bootstrap",
            "1",
            "--until-qpc",
            "100",
            "--qpc-frequency",
            "10",
            "--locale",
            "ja",
        ]
        .into_iter()
        .chain(extra.iter().copied())
        .map(OsString::from)
        .collect()
    }
    #[test]
    fn lexical_role_accepts_only_exact_original_counter_without_storage_paths()
    -> Result<(), Box<dyn Error>> {
        let role = ReporterRole::parse(values(&[]))?;
        assert_eq!(role.original.ticks(), 100);
        assert_eq!(role.original.frequency(), 10);
        assert_eq!(role.locale.as_str(), "ja");
        assert!(ReporterRole::parse(values(&["--storage-root", "E:\\private"])).is_err());
        let mut wrong = values(&[]);
        wrong[3] = "--qpc-frequency".into();
        assert!(ReporterRole::parse(wrong).is_err());
        let mut zero = values(&[]);
        zero[2] = "0".into();
        assert!(ReporterRole::parse(zero).is_err());
        let mut frequency = values(&[]);
        frequency[6] = "0".into();
        assert!(ReporterRole::parse(frequency).is_err());
        Ok(())
    }
    #[test]
    fn lexical_reporter_locale_rejects_missing_reordered_repeated_and_invalid_fields() {
        for invalid in ["", "unknown", "JA", " ja", "ja ", "日本語"] {
            let mut arguments = values(&[]);
            arguments[8] = invalid.into();
            assert!(ReporterRole::parse(arguments).is_err());
        }
        let mut absent = values(&[]);
        absent.truncate(7);
        assert!(ReporterRole::parse(absent).is_err());
        let mut missing = values(&[]);
        missing.pop();
        assert!(ReporterRole::parse(missing).is_err());
        let mut reordered = values(&[]);
        reordered.swap(5, 7);
        assert!(ReporterRole::parse(reordered).is_err());
        assert!(ReporterRole::parse(values(&["--locale", "ja"])).is_err());
    }
}
