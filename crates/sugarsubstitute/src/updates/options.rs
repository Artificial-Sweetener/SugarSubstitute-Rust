// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Parse exact local preparation inputs; feed/trust authority never comes from arguments.
use std::{ffi::OsString, io, path::PathBuf};
use sugarsubstitute_contracts::{
    application_launch::updater::preparation::ATTEMPT_ARGUMENT, configuration::LaunchConfiguration,
};
use sugarsubstitute_execution::windows_purpose_pipe::{
    InheritedLifecycleBootstrap, LifecyclePipeDeadline,
};
pub(super) struct Options {
    pub(super) installation: PathBuf,
    pub(super) workspace: PathBuf,
    /// Inherited update-attempt channel: bootstrap section and original handshake deadline.
    pub(super) attempt: (InheritedLifecycleBootstrap, LifecyclePipeDeadline),
}
impl Options {
    /// Validate the complete grammar before constructing an executor or opening any storage.
    /// The section is only adopted later, after the transport verifies its parent and purpose.
    pub(super) fn parse(arguments: impl IntoIterator<Item = OsString>) -> io::Result<Self> {
        let mut args = arguments.into_iter();
        let installation = path(args.next().ok_or_else(|| invalid("Update path missing"))?)?;
        let workspace = path(args.next().ok_or_else(|| invalid("Update path missing"))?)?;
        if args.next().as_deref() != Some(std::ffi::OsStr::new(ATTEMPT_ARGUMENT)) {
            return Err(invalid("Update attempt channel missing"));
        }
        let mut number = || -> io::Result<String> {
            args.next()
                .and_then(|value| value.into_string().ok())
                .filter(|value| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()))
                .ok_or_else(|| invalid("Update attempt channel field malformed"))
        };
        let token = number()?.parse().map_err(|_| invalid("Update attempt token malformed"))?;
        let ticks = number()?.parse().map_err(|_| invalid("Update attempt deadline malformed"))?;
        let frequency =
            number()?.parse().map_err(|_| invalid("Update attempt deadline malformed"))?;
        if args.next().is_some() {
            return Err(invalid("Unexpected update preparation argument"));
        }
        let bootstrap = InheritedLifecycleBootstrap::new(token).map_err(io::Error::other)?;
        let deadline = LifecyclePipeDeadline::new(ticks, frequency).map_err(io::Error::other)?;
        Ok(Self { installation, workspace, attempt: (bootstrap, deadline) })
    }
}

/// Share bounded native path grammar across preparation and activation entries.
pub(super) fn path(value: OsString) -> io::Result<PathBuf> {
    let value = PathBuf::from(value);
    LaunchConfiguration::validate_path(&value).map_err(io::Error::other)?;
    if value.capacity() > 256 * 1024 {
        return Err(invalid("Update path capacity exceeded"));
    }
    Ok(value)
}
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn parse(args: &[&str]) -> io::Result<Options> {
        Options::parse(args.iter().map(OsString::from))
    }
    const CHANNEL: [&str; 4] = [ATTEMPT_ARGUMENT, "8", "100", "10"];
    #[test]
    fn paths_are_exact_without_touching_disk() -> io::Result<()> {
        let options = parse(&[
            r"E:\unopened\app",
            r"E:\unopened\workspace",
            CHANNEL[0],
            CHANNEL[1],
            CHANNEL[2],
            CHANNEL[3],
        ])?;
        assert_eq!(options.installation, PathBuf::from(r"E:\unopened\app"));
        assert_eq!(options.workspace, PathBuf::from(r"E:\unopened\workspace"));
        Ok(())
    }
    #[test]
    fn incomplete_relative_and_authority_override_arguments_are_rejected() {
        for args in [
            vec![],
            vec![r"E:\app"],
            vec![r"E:\app", "relative", "release"],
            vec![r"E:\app", r"E:\work", "release"],
            vec![r"E:\app", r"E:\work", "release", "https://foreign.test/"],
        ] {
            assert!(parse(&args).is_err());
        }
    }
    #[test]
    fn the_update_attempt_channel_is_required_and_exact() {
        let paths = [r"E:\app", r"E:\work"];
        let with = |tail: &[&str]| parse(&[&paths[..], tail].concat());
        assert!(with(&[]).is_err());
        assert!(with(&CHANNEL[..3]).is_err());
        assert!(with(&["--other", "8", "100", "10"]).is_err());
        assert!(with(&[CHANNEL[0], "0", "100", "10"]).is_err());
        assert!(with(&[CHANNEL[0], "8", "-1", "10"]).is_err());
        assert!(with(&[CHANNEL[0], "8", "100", "0"]).is_err());
        assert!(with(&[CHANNEL[0], "8", "100", "10", "extra"]).is_err());
        assert!(with(&CHANNEL).is_ok());
    }
}
