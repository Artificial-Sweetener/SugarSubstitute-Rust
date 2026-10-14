// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Parse bounded inert development intent; actual registered owners authorize every effect.
#[cfg(all(test, feature = "k1-native-qualification"))]
#[path = "qualification/error_fixture.rs"]
pub(super) mod error_fixture;
#[cfg(feature = "k1-native-qualification")]
use std::ffi::OsStr;
use std::{error::Error, ffi::OsString, iter::Peekable};

/// Owned lexical intent grants no capture, presentation or filesystem authority.
/// Its private mode is assigned only to the first actually admitted original launch.
#[derive(Default)]
pub(in crate::production) struct QualificationIntent {
    #[cfg(feature = "k1-native-qualification")]
    mode: Option<Mode>,
}
#[cfg(feature = "k1-native-qualification")]
enum Mode {
    Fault,
    Preview(PreviewIntent),
}
/// Preserve an unchanged absolute campaign cutoff and a bounded owned D destination.
#[cfg(feature = "k1-native-qualification")]
pub(in crate::production) struct PreviewIntent {
    destination: std::path::PathBuf,
    deadline: sugarsubstitute_contracts::execution::CounterDeadline,
}
#[cfg(feature = "k1-native-qualification")]
impl PreviewIntent {
    /// Borrow the lexical destination; the real persister validates resolved ownership.
    #[cfg(test)]
    pub(in crate::production) fn destination(&self) -> &std::path::Path {
        &self.destination
    }
    /// Account the retained destination allocation inside the original qualification job.
    pub(in crate::production) fn allocated_bytes(&self) -> usize {
        self.destination.capacity()
    }
    /// Move the existing bounded destination allocation into the genuine preview worker input.
    pub(in crate::production) fn into_destination(self) -> std::path::PathBuf {
        self.destination
    }
    /// Borrow the original cutoff; callers decode it once through the central owner.
    pub(in crate::production) const fn deadline(
        &self,
    ) -> sugarsubstitute_contracts::execution::CounterDeadline {
        self.deadline
    }
}
impl QualificationIntent {
    /// Consume only a dedicated development prefix before the existing production grammar.
    /// # Errors
    /// Ordinary builds reject qualification; enabled builds reject malformed or mixed modes.
    pub(in crate::production) fn root<I: Iterator<Item = OsString>>(
        arguments: &mut Peekable<I>,
    ) -> Result<Self, Box<dyn Error>> {
        let Some(value) = arguments.peek() else {
            return Ok(Self::default());
        };
        if value != "--k1-native-fault-once" && value != "--k1-native-preview-once" {
            return Ok(Self::default());
        }
        #[cfg(not(feature = "k1-native-qualification"))]
        return Err("Native qualification requires the dedicated development build".into());
        #[cfg(feature = "k1-native-qualification")]
        {
            let mode = if arguments.next().as_deref() == Some(OsStr::new("--k1-native-fault-once"))
            {
                Mode::Fault
            } else {
                Mode::Preview(preview(arguments)?)
            };
            if arguments.peek().is_none_or(|next| next != "--production") {
                return Err(
                    "Native qualification requires the original production invocation".into()
                );
            }
            Ok(Self { mode: Some(mode) })
        }
    }
    /// Decode a terminal Desktop-only suffix; inherited role spelling grants no authority.
    /// # Errors
    /// Rejects unknown, repeated, cross-role or ordinary-build qualification suffixes.
    pub(in crate::production) fn child(
        arguments: &mut impl Iterator<Item = OsString>,
    ) -> Result<Self, Box<dyn Error>> {
        let Some(flag) = arguments.next() else {
            return Ok(Self::default());
        };
        #[cfg(not(feature = "k1-native-qualification"))]
        {
            let _ = flag;
            Err("Unexpected desktop role arguments".into())
        }
        #[cfg(feature = "k1-native-qualification")]
        {
            let mode = if flag == "--internal-k1-native-fault" {
                Mode::Fault
            } else if flag == "--internal-k1-native-preview" {
                Mode::Preview(preview(arguments)?)
            } else {
                return Err("Unexpected desktop qualification argument".into());
            };
            if arguments.next().is_some() {
                return Err("Repeated desktop qualification arguments".into());
            }
            Ok(Self { mode: Some(mode) })
        }
    }
    /// Encode only this original launch's inert intent under its already-funded projection.
    /// # Errors
    /// Retains allocation failure without granting role or registration authority.
    pub(in crate::production) fn append(
        &self,
        arguments: &mut Vec<Vec<u16>>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        #[cfg(feature = "k1-native-qualification")]
        {
            let values: Vec<OsString> = match &self.mode {
                None => return Ok(()),
                Some(Mode::Fault) => vec!["--internal-k1-native-fault".into()],
                Some(Mode::Preview(intent)) => vec![
                    "--internal-k1-native-preview".into(),
                    "--preview-until-qpc".into(),
                    intent.deadline.ticks().to_string().into(),
                    "--preview-qpc-frequency".into(),
                    intent.deadline.frequency().to_string().into(),
                    "--preview-destination".into(),
                    intent.destination.as_os_str().to_owned(),
                ],
            };
            arguments.try_reserve_exact(values.len())?;
            use std::os::windows::ffi::OsStrExt;
            for value in values {
                let mut units = Vec::new();
                units.try_reserve_exact(value.encode_wide().count())?;
                units.extend(value.encode_wide());
                arguments.push(units);
            }
        }
        #[cfg(not(feature = "k1-native-qualification"))]
        let _ = arguments;
        Ok(())
    }
    /// Read only the inert fault request; the execution owner independently authenticates capture.
    #[cfg(feature = "k1-native-qualification")]
    pub(in crate::production) fn fault(&self) -> bool {
        matches!(self.mode, Some(Mode::Fault))
    }
    /// Borrow passive preview intent for original capture storage preflight; no authority is issued.
    #[cfg(feature = "k1-native-qualification")]
    pub(in crate::production) fn preview(&self) -> Option<&PreviewIntent> {
        match &self.mode {
            Some(Mode::Preview(intent)) => Some(intent),
            _ => None,
        }
    }
    /// Transfer the original preview request once without renewing its cutoff.
    #[cfg(feature = "k1-native-qualification")]
    pub(in crate::production) fn take_preview(&mut self) -> Option<PreviewIntent> {
        if !matches!(self.mode, Some(Mode::Preview(_))) {
            return None;
        }
        match self.mode.take() {
            Some(Mode::Preview(intent)) => Some(intent),
            _ => None,
        }
    }
}
#[cfg(feature = "k1-native-qualification")]
fn preview(
    arguments: &mut impl Iterator<Item = OsString>,
) -> Result<PreviewIntent, Box<dyn Error>> {
    use std::os::windows::ffi::OsStrExt;
    use sugarsubstitute_contracts::execution::CounterDeadline;
    let ticks = field(arguments, "--preview-until-qpc")?
        .into_string()
        .map_err(|_| "Preview cutoff encoding")?
        .parse()?;
    let frequency = field(arguments, "--preview-qpc-frequency")?
        .into_string()
        .map_err(|_| "Preview frequency encoding")?
        .parse()?;
    let deadline = CounterDeadline::new(ticks, frequency)?;
    let destination = std::path::PathBuf::from(field(arguments, "--preview-destination")?);
    if destination.capacity() > 4096 {
        return Err("Preview destination allocation exceeds its original funding".into());
    }
    let units: Vec<_> = destination.as_os_str().encode_wide().take(1025).collect();
    if units.len() < 4
        || units.len() > 1024
        || units.contains(&0)
        || !matches!(units[0], 68 | 100)
        || units[1] != 58
        || units[2] != 92
        || units[3..].contains(&58)
        || !destination.is_absolute()
        || destination.components().any(|part| {
            matches!(part, std::path::Component::ParentDir | std::path::Component::CurDir)
        })
    {
        return Err("Preview destination must be a bounded absolute D path".into());
    }
    sugarsubstitute_contracts::configuration::LaunchConfiguration::validate_path(&destination)?;
    Ok(PreviewIntent { destination, deadline })
}
#[cfg(feature = "k1-native-qualification")]
fn field(
    arguments: &mut impl Iterator<Item = OsString>,
    name: &str,
) -> Result<OsString, Box<dyn Error>> {
    if arguments.next().as_deref() != Some(OsStr::new(name)) {
        return Err("Qualification field order mismatch".into());
    }
    arguments.next().ok_or_else(|| "Qualification field missing".into())
}

#[cfg(test)]
#[path = "qualification/tests.rs"]
mod tests;
