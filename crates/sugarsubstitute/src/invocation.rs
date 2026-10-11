// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate implemented executable roles before native presentation or process effects.

use std::{error::Error, ffi::OsString, path::PathBuf, time::Duration};
use sugarsubstitute_splash::{Appearance, Theme};

/// Select the implemented splash proof or an isolated immediate native-exit fixture.
pub(crate) enum Invocation {
    /// Existing interactive/off-screen demonstration inputs.
    Demo(DemoOptions),
    /// Windows qualification-only process exit; no splash or child process is created.
    #[cfg(target_os = "windows")]
    NativeExitFixture(i32),
}
impl Invocation {
    /// Parse process arguments without starting native or execution resources.
    pub(crate) fn parse() -> Result<Self, Box<dyn Error>> {
        Self::parse_arguments(std::env::args_os().skip(1))
    }

    /// Keep internal fixture selection separate from demonstration presentation options.
    fn parse_arguments(
        arguments: impl IntoIterator<Item = OsString>,
    ) -> Result<Self, Box<dyn Error>> {
        let arguments = arguments.into_iter();
        #[cfg(target_os = "windows")]
        let mut arguments = arguments.peekable();
        #[cfg(target_os = "windows")]
        if arguments.peek().is_some_and(|argument| argument == "--internal-native-exit-fixture") {
            arguments.next();
            let code = arguments.next().ok_or("Native exit fixture requires an exit code")?;
            let code = match code.to_str() {
                Some("0") => 0,
                Some("37") => 37,
                Some("259") => 259,
                _ => return Err("Unsupported native exit fixture code".into()),
            };
            if arguments.next().is_some() {
                return Err("Native exit fixture rejects additional arguments".into());
            }
            return Ok(Self::NativeExitFixture(code));
        }
        Ok(Self::Demo(DemoOptions::parse_arguments(arguments)?))
    }
}

/// Hold explicit demonstration and qualification invocation inputs.
pub(crate) struct DemoOptions {
    /// Initial native disclosure and theme state.
    pub(crate) appearance: Appearance,
    /// Requested locale, resolved by the canonical localization owner.
    pub(crate) locale: String,
    /// Disable scheduled character animation.
    pub(crate) reduced_motion: bool,
    /// Optional native proof lifetime.
    pub(crate) duration: Option<Duration>,
    /// Native qualification receipt destination.
    pub(crate) receipt: Option<PathBuf>,
    /// Optional first-frame pixel destination.
    pub(crate) frame_audit: Option<PathBuf>,
    /// Collect bounded native timing milestones.
    pub(crate) timing_audit: bool,
    /// Offline capture scenario input.
    pub(crate) scenarios: Option<PathBuf>,
    /// Offline frame export destination.
    pub(crate) output: Option<PathBuf>,
}

impl DemoOptions {
    /// Validate explicit qualification inputs before constructing any native or execution owner.
    fn parse_arguments(
        arguments: impl IntoIterator<Item = std::ffi::OsString>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut options = Self {
            appearance: Appearance::demo(),
            locale: "en".into(),
            reduced_motion: false,
            duration: None,
            receipt: None,
            frame_audit: None,
            timing_audit: false,
            scenarios: None,
            output: None,
        };
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            match argument.to_str().ok_or("Option name is not valid Unicode")? {
                "--collapsed" => options.appearance.expanded = false,
                "--light" => options.appearance.theme = Theme::Light,
                "--reduced-motion" => options.reduced_motion = true,
                "--timing-audit" => options.timing_audit = true,
                "--locale" => {
                    options.locale = arguments
                        .next()
                        .ok_or("--locale requires a value")?
                        .into_string()
                        .map_err(|_| "Locale is not valid Unicode")?
                }
                "--duration-ms" => {
                    let value = arguments.next().ok_or("--duration-ms requires a value")?;
                    let milliseconds =
                        value.to_str().ok_or("Duration is not valid Unicode")?.parse()?;
                    options.duration = Some(Duration::from_millis(milliseconds));
                }
                "--receipt" => {
                    options.receipt =
                        Some(arguments.next().ok_or("--receipt requires a path")?.into())
                }
                "--frame-audit" => {
                    options.frame_audit =
                        Some(arguments.next().ok_or("--frame-audit requires a path")?.into())
                }
                "--scenarios" => {
                    options.scenarios =
                        Some(arguments.next().ok_or("--scenarios requires a path")?.into())
                }
                "--output" => {
                    options.output =
                        Some(arguments.next().ok_or("--output requires a path")?.into())
                }
                _ => return Err("Unknown SugarSubstitute proof option".into()),
            }
        }
        if options.scenarios.is_some() != options.output.is_some() {
            return Err("--scenarios and --output must be supplied together".into());
        }
        if options.frame_audit.is_some()
            && (options.receipt.is_none() || options.scenarios.is_some())
        {
            return Err("--frame-audit requires --receipt in native qualification mode".into());
        }
        if options.timing_audit && (options.receipt.is_none() || options.scenarios.is_some()) {
            return Err("--timing-audit requires --receipt in native qualification mode".into());
        }
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Preserve the native proof's combined presentation and receipt options.
    #[test]
    fn demonstration_options_keep_their_invocation_role() -> Result<(), Box<dyn Error>> {
        let invocation = Invocation::parse_arguments(
            [
                "--light",
                "--collapsed",
                "--reduced-motion",
                "--locale",
                "ja",
                "--duration-ms",
                "750",
                "--timing-audit",
                "--receipt",
                "receipt.json",
            ]
            .map(OsString::from),
        )?;
        let options = match invocation {
            Invocation::Demo(options) => options,
            #[cfg(target_os = "windows")]
            Invocation::NativeExitFixture(_) => {
                return Err("Presentation options selected an exit fixture".into());
            }
        };
        assert_eq!(options.appearance.theme, Theme::Light);
        assert!(!options.appearance.expanded);
        assert!(options.reduced_motion);
        assert_eq!(options.locale, "ja");
        assert_eq!(options.duration, Some(Duration::from_millis(750)));
        assert!(options.timing_audit);
        assert_eq!(options.receipt, Some(PathBuf::from("receipt.json")));
        Ok(())
    }

    /// Exit 259 is a genuine signaled exit despite sharing STILL_ACTIVE's numeric value.
    #[cfg(target_os = "windows")]
    #[test]
    fn native_exit_fixture_preserves_all_supported_windows_codes() -> Result<(), Box<dyn Error>> {
        for (argument, expected) in [("0", 0), ("37", 37), ("259", 259)] {
            let invocation = Invocation::parse_arguments(
                ["--internal-native-exit-fixture", argument].map(OsString::from),
            )?;
            assert!(matches!(invocation, Invocation::NativeExitFixture(code) if code == expected));
        }
        Ok(())
    }

    /// Fixture selection rejects missing, extra, unsupported and non-Unicode input.
    #[cfg(target_os = "windows")]
    #[test]
    fn native_exit_fixture_rejects_ambiguous_invocations() {
        use std::os::windows::ffi::OsStringExt;

        for arguments in [
            vec!["--internal-native-exit-fixture"],
            vec!["--internal-native-exit-fixture", "1"],
            vec!["--internal-native-exit-fixture", "-1"],
            vec!["--internal-native-exit-fixture", "0259"],
            vec!["--internal-native-exit-fixture", "37", "--light"],
            vec!["--light", "--internal-native-exit-fixture", "0"],
        ] {
            assert!(
                Invocation::parse_arguments(arguments.into_iter().map(OsString::from)).is_err()
            );
        }
        assert!(
            Invocation::parse_arguments([
                OsString::from("--internal-native-exit-fixture"),
                OsString::from_wide(&[0xd800]),
            ])
            .is_err()
        );
    }

    /// Timing evidence requires a native receipt destination and cannot silently disappear offline.
    #[test]
    fn timing_qualification_rejects_missing_receipt_and_offline_export()
    -> Result<(), Box<dyn Error>> {
        let arguments =
            |values: &[&str]| values.iter().map(std::ffi::OsString::from).collect::<Vec<_>>();
        assert!(DemoOptions::parse_arguments(arguments(&["--timing-audit"])).is_err());
        assert!(
            DemoOptions::parse_arguments(arguments(&[
                "--timing-audit",
                "--receipt",
                "receipt.json",
                "--scenarios",
                "inputs.json",
                "--output",
                "frames",
            ]))
            .is_err()
        );
        let options = DemoOptions::parse_arguments(arguments(&[
            "--timing-audit",
            "--receipt",
            "receipt.json",
        ]))?;
        assert!(options.timing_audit);
        assert_eq!(options.receipt, Some(PathBuf::from("receipt.json")));
        assert!(options.frame_audit.is_none());
        Ok(())
    }
}
