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
    /// Windows same-executable output qualification receipt destination.
    #[cfg(target_os = "windows")]
    NativeOutputFixture(PathBuf),
    /// Fixed source-write/held-writer role with explicit optional bootstrap.
    #[cfg(target_os = "windows")]
    NativeOutputChild(crate::output_fixture::ChildOptions),
    /// Windows headless same-executable registry qualification receipt destination.
    #[cfg(target_os = "windows")]
    NativeRegistryFixture(PathBuf),
    /// Windows child with four explicit inherited fixture events and one original deadline.
    #[cfg(target_os = "windows")]
    NativeHeldChild(crate::held_child::ChildOptions),
    /// Windows held64 cancellation campaign receipt destination.
    #[cfg(target_os = "windows")]
    NativeHeldRegistryFixture(PathBuf),
    /// Windows forwarding root; fixed child inherits the retained outer Job.
    #[cfg(target_os = "windows")]
    NativeForwardingRoot(crate::held_child::ChildOptions),
    /// Windows root-exit/surviving-descendant campaign receipt destination.
    #[cfg(target_os = "windows")]
    NativeDescendantFixture(PathBuf),
    /// Windows authenticated lifecycle pipe campaign receipt destination.
    #[cfg(target_os = "windows")]
    NativePipeFixture(PathBuf),
    /// Windows child admitted before adopting its single read-only bootstrap section.
    #[cfg(target_os = "windows")]
    NativePipeChild(crate::pipe_fixture::ChildOptions),
    /// Actual cross-process claim qualification with an isolated D-drive workspace.
    #[cfg(target_os = "windows")]
    NativeClaimFixture(crate::claim_fixture::CampaignOptions),
    /// Child reserves its original native deadline before fallible claim effects.
    #[cfg(target_os = "windows")]
    NativeClaimChild(crate::claim_fixture::ChildOptions),
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
        if arguments.peek().is_some_and(|argument| argument == "--internal-native-output-child") {
            arguments.next();
            return Ok(Self::NativeOutputChild(crate::output_fixture::ChildOptions::parse(
                arguments,
            )?));
        }
        #[cfg(target_os = "windows")]
        if arguments.peek().is_some_and(|argument| argument == "--internal-native-output-fixture") {
            arguments.next();
            if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--receipt")) {
                return Err("Output fixture requires --receipt".into());
            }
            let path = PathBuf::from(arguments.next().ok_or("Output fixture receipt missing")?);
            if !path.is_absolute() || arguments.next().is_some() {
                return Err("Output fixture requires one absolute receipt path".into());
            }
            return Ok(Self::NativeOutputFixture(path));
        }
        #[cfg(target_os = "windows")]
        if arguments.peek().is_some_and(|argument| argument == "--internal-native-forwarding-root")
        {
            arguments.next();
            return Ok(Self::NativeForwardingRoot(crate::held_child::ChildOptions::parse(
                arguments,
            )?));
        }
        #[cfg(target_os = "windows")]
        if arguments
            .peek()
            .is_some_and(|argument| argument == "--internal-native-descendant-fixture")
        {
            arguments.next();
            if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--receipt")) {
                return Err("Descendant fixture requires --receipt".into());
            }
            let path =
                PathBuf::from(arguments.next().ok_or("Descendant fixture receipt path missing")?);
            if !path.is_absolute() || arguments.next().is_some() {
                return Err("Descendant fixture requires one absolute receipt path".into());
            }
            return Ok(Self::NativeDescendantFixture(path));
        }
        #[cfg(target_os = "windows")]
        if arguments.peek().is_some_and(|argument| argument == "--internal-native-claim-fixture") {
            arguments.next();
            return Ok(Self::NativeClaimFixture(crate::claim_fixture::CampaignOptions::parse(
                arguments,
            )?));
        }
        #[cfg(target_os = "windows")]
        if arguments.peek().is_some_and(|argument| argument == "--internal-native-claim-child") {
            arguments.next();
            return Ok(Self::NativeClaimChild(crate::claim_fixture::ChildOptions::parse(
                arguments,
            )?));
        }
        #[cfg(target_os = "windows")]
        if arguments.peek().is_some_and(|argument| argument == "--internal-native-held-child") {
            arguments.next();
            return Ok(Self::NativeHeldChild(crate::held_child::ChildOptions::parse(arguments)?));
        }
        #[cfg(target_os = "windows")]
        if arguments.peek().is_some_and(|argument| argument == "--internal-native-pipe-child") {
            arguments.next();
            return Ok(Self::NativePipeChild(crate::pipe_fixture::ChildOptions::parse(arguments)?));
        }
        #[cfg(target_os = "windows")]
        let pipe =
            arguments.peek().is_some_and(|argument| argument == "--internal-native-pipe-fixture");
        #[cfg(target_os = "windows")]
        let held = arguments
            .peek()
            .is_some_and(|argument| argument == "--internal-native-held-registry-fixture");
        #[cfg(target_os = "windows")]
        if held
            || pipe
            || arguments
                .peek()
                .is_some_and(|argument| argument == "--internal-native-registry-fixture")
        {
            arguments.next();
            if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--receipt")) {
                return Err("Native registry fixture requires --receipt".into());
            }
            let path =
                PathBuf::from(arguments.next().ok_or("Native registry fixture requires a path")?);
            if !path.is_absolute() || arguments.next().is_some() {
                return Err("Native registry fixture requires one absolute receipt path".into());
            }
            return Ok(if pipe {
                Self::NativePipeFixture(path)
            } else if held {
                Self::NativeHeldRegistryFixture(path)
            } else {
                Self::NativeRegistryFixture(path)
            });
        }
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
mod tests;
