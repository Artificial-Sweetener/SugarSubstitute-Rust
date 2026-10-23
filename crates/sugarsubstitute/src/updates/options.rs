// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Parse exact local preparation inputs; feed/trust authority never comes from arguments.
use std::{ffi::OsString, io, path::PathBuf};
use sugarsubstitute_contracts::configuration::LaunchConfiguration;
pub(super) struct Options {
    pub(super) installation: PathBuf,
    pub(super) workspace: PathBuf,
}
impl Options {
    /// Validate the complete grammar before constructing an executor or opening any storage.
    pub(super) fn parse(arguments: impl IntoIterator<Item = OsString>) -> io::Result<Self> {
        let mut args = arguments.into_iter();
        let mut path = || -> io::Result<PathBuf> {
            let value = PathBuf::from(args.next().ok_or_else(|| invalid("Update path missing"))?);
            LaunchConfiguration::validate_path(&value).map_err(io::Error::other)?;
            if value.capacity() > 256 * 1024 {
                return Err(invalid("Update path capacity exceeded"));
            }
            Ok(value)
        };
        let installation = path()?;
        let workspace = path()?;
        if args.next().is_some() {
            return Err(invalid("Unexpected update preparation argument"));
        }
        Ok(Self { installation, workspace })
    }
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
    #[test]
    fn paths_are_exact_without_touching_disk() -> io::Result<()> {
        let options = parse(&[r"E:\unopened\app", r"E:\unopened\workspace"])?;
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
}
