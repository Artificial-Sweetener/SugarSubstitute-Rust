// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pin exact selected startup documents while their existing configuration owners validate them.
use crate::durable_storage::{
    check,
    files::{self, Directory},
    invalid,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};
use sugarsubstitute_configuration::{
    descriptor::LaunchDescriptor,
    paths::{ConfigurationSource, StartupPaths},
    startup::{MAX_STARTUP_CONFIGURATION_BYTES, parse_startup_configuration},
};
use sugarsubstitute_contracts::{configuration::StartupConfiguration, execution::JobContext};

/// Exact original roles retained independently of their current decoded values.
#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Kind {
    Descriptor,
    Startup,
    Ready,
}
impl Kind {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Descriptor => "descriptor",
            Self::Startup => "startup",
            Self::Ready => "ready",
        }
    }
    pub(super) fn limit(self) -> usize {
        match self {
            Self::Ready => 1024 * 1024,
            Self::Descriptor | Self::Startup => MAX_STARTUP_CONFIGURATION_BYTES,
        }
    }
    pub(super) fn extension(self) -> &'static str {
        match self {
            Self::Startup => "toml",
            Self::Descriptor | Self::Ready => "json",
        }
    }
}

/// One complete original remains borrowed from native file/ancestry custody through capture.
pub(super) struct ControlFile {
    pub(super) path: PathBuf,
    pub(super) bytes: Vec<u8>,
    pub(super) file: File,
    _parent: Directory,
}
impl ControlFile {
    /// Read bounded bytes without following links, changing permissions or creating an input.
    pub(super) fn read(path: &Path, limit: usize, context: &JobContext) -> io::Result<Self> {
        check(context)?;
        files::lexical(path)?;
        let parent = Directory::open(
            path.parent().ok_or_else(|| invalid("Launch input parent absent"))?,
            false,
        )?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid("Launch input name is not Unicode"))?;
        let mut file = parent.read(name)?;
        files::single_link(&file)?;
        let bytes = files::read_admitted(&mut file, limit, context)?;
        let path = files::canonical_path(path, context)?;
        check(context)?;
        Ok(Self { path, bytes, file, _parent: parent })
    }
}

/// Each successfully read original is adopted before parsing or opening its next dependency.
#[derive(Default)]
pub(super) struct Controls {
    pub(super) descriptor: Option<ControlFile>,
    pub(super) startup: Option<ControlFile>,
    pub(super) ready: Option<ControlFile>,
}
impl Controls {
    /// Consume the same inert input source used by ordinary startup; codecs stay with configuration.
    pub(super) fn resolve(
        &mut self,
        source: ConfigurationSource,
        context: &JobContext,
    ) -> io::Result<(StartupPaths, StartupConfiguration)> {
        let paths = match source {
            ConfigurationSource::Explicit(paths) => paths,
            ConfigurationSource::Installed(path) => {
                self.descriptor =
                    Some(ControlFile::read(&path, MAX_STARTUP_CONFIGURATION_BYTES, context)?);
                let original = self
                    .descriptor
                    .as_ref()
                    .ok_or_else(|| invalid("Original descriptor custody absent"))?;
                let descriptor = LaunchDescriptor::parse(&original.bytes)?;
                check(context)?;
                descriptor.into_startup_paths().map_err(io::Error::other)?
            }
        };
        self.startup = Some(ControlFile::read(
            paths.configuration(),
            MAX_STARTUP_CONFIGURATION_BYTES,
            context,
        )?);
        let original =
            self.startup.as_ref().ok_or_else(|| invalid("Original startup custody absent"))?;
        let parsed = parse_startup_configuration(&original.bytes).map_err(io::Error::other)?;
        check(context)?;
        Ok((paths, parsed))
    }

    /// Lend only complete captured originals, never fabricated readiness or replacement bytes.
    pub(super) fn files(&self) -> impl Iterator<Item = (Kind, &ControlFile)> {
        [
            (Kind::Descriptor, self.descriptor.as_ref()),
            (Kind::Startup, self.startup.as_ref()),
            (Kind::Ready, self.ready.as_ref()),
        ]
        .into_iter()
        .filter_map(|(kind, file)| file.map(|file| (kind, file)))
    }
}
