// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded invocation payloads and borrow exact decoded Windows units.

mod originals;
mod owned;
pub use originals::EntryOriginalsReference;
pub use owned::{EntryLaunchParts, EntryLaunchRequest, MAX_ENTRY_REQUEST_ALLOCATION_BYTES};

use super::EntryShapeError;
use crate::configuration::{
    CanvasRendererPreference, LaunchConfiguration, LoopbackOrigin, MAX_CANVAS_RENDERER_BYTES,
    Presentation, ProfileId,
};
use std::{fmt, num::NonZeroU16};

/// Maximum raw Windows units in a nonterminated entry path.
pub const MAX_ENTRY_PATH_UNITS: usize = 32766;
/// Maximum units including the original double-NUL environment terminator.
pub const MAX_ENTRY_ENVIRONMENT_UNITS: usize = 32767;

fn validate_path_units(units: impl Iterator<Item = u16>) -> Result<(), EntryShapeError> {
    let mut count = 0;
    for unit in units {
        if unit == 0 || count == MAX_ENTRY_PATH_UNITS {
            return Err(EntryShapeError::Path);
        }
        count += 1;
    }
    if count == 0 {
        return Err(EntryShapeError::Path);
    }
    Ok(())
}
fn units(bytes: &[u8]) -> impl ExactSizeIterator<Item = u16> + Clone + '_ {
    bytes.chunks_exact(2).map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
}
fn validate_environment(units: impl Iterator<Item = u16>) -> Result<(), EntryShapeError> {
    let mut length = 0;
    let mut separator = false;
    let mut empty_start = false;
    let mut closed = false;
    for (index, unit) in units.enumerate() {
        if index >= MAX_ENTRY_ENVIRONMENT_UNITS || closed {
            return Err(EntryShapeError::Environment);
        }
        if unit == 0 {
            if length == 0 {
                if index == 0 {
                    empty_start = true;
                } else {
                    closed = true;
                }
            } else {
                if !separator {
                    return Err(EntryShapeError::Environment);
                }
                length = 0;
                separator = false;
            }
        } else {
            if empty_start {
                return Err(EntryShapeError::Environment);
            }
            separator |= unit == u16::from(b'=') && length > 0;
            length += 1;
        }
    }
    if !closed {
        return Err(EntryShapeError::Environment);
    }
    Ok(())
}

/// Borrow bounded nonterminated Windows path units, including unpaired surrogates.
/// Raw shape grants no lexical or native trust; owned conversion reruns path policy.
#[derive(Clone, Copy)]
pub struct EntryWindowsPathView<'a>(&'a [u8]);
impl<'a> EntryWindowsPathView<'a> {
    /// Validate exact little-endian units without allocating or scalar normalization.
    /// # Errors
    /// Rejects odd byte length, empty/excessive units or NUL.
    pub fn from_le_bytes(bytes: &'a [u8]) -> Result<Self, EntryShapeError> {
        if !bytes.len().is_multiple_of(2) {
            return Err(EntryShapeError::Path);
        }
        validate_path_units(units(bytes))?;
        Ok(Self(bytes))
    }
    /// Borrow the original little-endian bytes.
    pub fn le_bytes(self) -> &'a [u8] {
        self.0
    }
    /// Return exact raw UTF-16 unit count.
    pub fn unit_len(self) -> usize {
        self.0.len() / 2
    }
    /// Iterate original units without allocation or lossy decoding.
    pub fn units(self) -> impl ExactSizeIterator<Item = u16> + Clone + 'a {
        units(self.0)
    }
}
/// Borrow the exact original bounded environment block including its terminator.
#[derive(Clone, Copy)]
pub struct EntryEnvironmentView<'a>(&'a [u8]);
impl<'a> EntryEnvironmentView<'a> {
    /// Validate exact little-endian environment snapshot structure without allocation.
    /// # Errors
    /// Rejects malformed entries, terminator, odd length or excessive units.
    pub fn from_le_bytes(bytes: &'a [u8]) -> Result<Self, EntryShapeError> {
        if !bytes.len().is_multiple_of(2) {
            return Err(EntryShapeError::Environment);
        }
        validate_environment(units(bytes))?;
        Ok(Self(bytes))
    }
    /// Borrow unchanged original little-endian environment bytes.
    pub fn le_bytes(self) -> &'a [u8] {
        self.0
    }
    /// Return the exact number of units including double NUL.
    pub fn unit_len(self) -> usize {
        self.0.len() / 2
    }
    /// Iterate exact units without reordering variables or normalizing surrogates.
    pub fn units(self) -> impl ExactSizeIterator<Item = u16> + Clone + 'a {
        units(self.0)
    }
}

/// Borrow an exactly validated length-prefixed UTF-8 argument section.
#[derive(Clone, Copy)]
pub struct EntryArgumentsView<'a> {
    bytes: &'a [u8],
    count: u8,
}
impl<'a> EntryArgumentsView<'a> {
    /// Validate each u32 LE length and exact UTF-8 payload, consuming the complete section.
    /// # Errors
    /// Preserves authoritative argument grammar errors; rejects malformed encoding.
    pub fn from_encoded(bytes: &'a [u8], count: u8) -> Result<Self, EntryShapeError> {
        let view = Self { bytes, count };
        let mut iterator = view.iter();
        for _ in 0..count {
            if iterator.next().is_none() {
                return Err(EntryShapeError::Arguments);
            }
        }
        if !iterator.bytes.is_empty() {
            return Err(EntryShapeError::Arguments);
        }
        LaunchConfiguration::validate_arguments(view.iter())
            .map_err(EntryShapeError::Configuration)?;
        Ok(view)
    }
    /// Borrow unchanged exact encoded argument bytes, excluding external count.
    pub fn encoded(self) -> &'a [u8] {
        self.bytes
    }
    /// Return separately passed argument count.
    pub fn len(self) -> usize {
        usize::from(self.count)
    }
    /// Return whether the section carries no arguments.
    pub fn is_empty(self) -> bool {
        self.count == 0
    }
    /// Iterate validated original argument slices without allocation.
    pub fn iter(self) -> EntryArgumentsIter<'a> {
        EntryArgumentsIter { bytes: self.bytes, remaining: self.count }
    }
}
/// Iterate immutable constructor-validated argument bytes without copying.
#[derive(Clone)]
pub struct EntryArgumentsIter<'a> {
    bytes: &'a [u8],
    remaining: u8,
}
impl<'a> Iterator for EntryArgumentsIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let prefix: [u8; 4] = self.bytes.get(..4)?.try_into().ok()?;
        let length = usize::try_from(u32::from_le_bytes(prefix)).ok()?;
        let end = 4_usize.checked_add(length)?;
        let text = std::str::from_utf8(self.bytes.get(4..end)?).ok()?;
        self.bytes = self.bytes.get(end..)?;
        self.remaining -= 1;
        Some(text)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = usize::from(self.remaining);
        (count, Some(count))
    }
}
impl ExactSizeIterator for EntryArgumentsIter<'_> {}

/// Borrow exact managed startup inputs or an already validated attached origin.
#[derive(Clone, Copy)]
pub enum EntryConnectionView<'a> {
    /// Lexical raw paths grant no executable trust before charged owned validation.
    Managed {
        /// Original requested backend root units.
        root: EntryWindowsPathView<'a>,
        /// Original requested interpreter units.
        interpreter: EntryWindowsPathView<'a>,
        /// Exact original UTF-8 argument section.
        arguments: EntryArgumentsView<'a>,
        /// Explicit requested nonzero port.
        port: NonZeroU16,
    },
    /// Explicit externally owned loopback service.
    Attached(LoopbackOrigin),
}
/// Four independently selected original frontend locations.
#[derive(Clone, Copy)]
pub struct EntryFrontendView<'a> {
    package: EntryWindowsPathView<'a>,
    metadata: EntryWindowsPathView<'a>,
    cube: EntryWindowsPathView<'a>,
    script: EntryWindowsPathView<'a>,
}
impl<'a> EntryFrontendView<'a> {
    /// Compose already bounded original location slices without copying.
    pub const fn new(
        package: EntryWindowsPathView<'a>,
        metadata: EntryWindowsPathView<'a>,
        cube: EntryWindowsPathView<'a>,
        script: EntryWindowsPathView<'a>,
    ) -> Self {
        Self { package, metadata, cube, script }
    }
    /// Borrow requested package directory units.
    pub const fn package_directory(self) -> EntryWindowsPathView<'a> {
        self.package
    }
    /// Borrow requested METADATA file units.
    pub const fn metadata_file(self) -> EntryWindowsPathView<'a> {
        self.metadata
    }
    /// Borrow requested SugarCubes directory units.
    pub const fn cube_directory(self) -> EntryWindowsPathView<'a> {
        self.cube
    }
    /// Borrow requested backend script units.
    pub const fn backend_script(self) -> EntryWindowsPathView<'a> {
        self.script
    }
}
/// Borrowed stored canvas renderer preference carried by one launch request.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EntryCanvasRendererView<'a> {
    /// No preference is stored.
    #[default]
    Absent,
    /// Stored selector text of at most 64 bytes.
    Stored(&'a str),
    /// A stored value that is not bounded selector text.
    Malformed,
}
impl EntryCanvasRendererView<'_> {
    /// Own the borrowed preference for startup configuration.
    pub fn to_owned_preference(self) -> CanvasRendererPreference {
        match self {
            Self::Absent => CanvasRendererPreference::Absent,
            Self::Stored(text) => CanvasRendererPreference::stored(text),
            Self::Malformed => CanvasRendererPreference::Malformed,
        }
    }
}
/// Exact original borrowed payload; no allocation, privilege or native proof.
#[derive(Clone, Copy)]
pub struct EntryLaunchRequestView<'a> {
    connection: EntryConnectionView<'a>,
    frontend: EntryFrontendView<'a>,
    profile_directory: EntryWindowsPathView<'a>,
    profile: &'a str,
    presentation: Presentation,
    environment: EntryEnvironmentView<'a>,
    canvas_renderer: EntryCanvasRendererView<'a>,
    originals: Option<EntryOriginalsReference>,
}
impl<'a> EntryLaunchRequestView<'a> {
    /// Compose bounded slices and validate the profile through its authoritative owner.
    /// # Errors
    /// Returns the unchanged profile grammar error. Wire whole-frame preflight belongs to execution.
    pub fn new(
        connection: EntryConnectionView<'a>,
        frontend: EntryFrontendView<'a>,
        profile_directory: EntryWindowsPathView<'a>,
        profile: &'a str,
        presentation: Presentation,
        environment: EntryEnvironmentView<'a>,
    ) -> Result<Self, EntryShapeError> {
        ProfileId::validate(profile).map_err(EntryShapeError::Configuration)?;
        Ok(Self {
            connection,
            frontend,
            profile_directory,
            profile,
            presentation,
            environment,
            canvas_renderer: EntryCanvasRendererView::Absent,
            originals: None,
        })
    }
    /// Attach the stored canvas renderer preference.
    /// # Errors
    /// Rejects stored text above the 64-byte selector bound.
    pub fn with_canvas_renderer(
        mut self,
        preference: EntryCanvasRendererView<'a>,
    ) -> Result<Self, EntryShapeError> {
        if matches!(preference, EntryCanvasRendererView::Stored(text) if text.len() > MAX_CANVAS_RENDERER_BYTES)
        {
            return Err(EntryShapeError::CanvasRenderer);
        }
        self.canvas_renderer = preference;
        Ok(self)
    }
    /// Return the stored canvas renderer preference.
    pub const fn canvas_renderer(self) -> EntryCanvasRendererView<'a> {
        self.canvas_renderer
    }
    /// Attach inert matching data for separately retained passive original bytes.
    /// This performs no native admission and cannot authorize a supplied path or migration.
    pub const fn with_originals(mut self, reference: EntryOriginalsReference) -> Self {
        self.originals = Some(reference);
        self
    }
    /// Observe the fixed original-snapshot commitment without acquiring file custody.
    pub const fn originals(self) -> Option<EntryOriginalsReference> {
        self.originals
    }
    /// Return exact original requested connection mode.
    pub const fn connection(self) -> EntryConnectionView<'a> {
        self.connection
    }
    /// Return exact original frontend location slices.
    pub const fn frontend(self) -> EntryFrontendView<'a> {
        self.frontend
    }
    /// Return exact original browser profile directory units.
    pub const fn profile_directory(self) -> EntryWindowsPathView<'a> {
        self.profile_directory
    }
    /// Borrow original validated profile spelling.
    pub const fn profile(self) -> &'a str {
        self.profile
    }
    /// Return validated locale, appearance and motion choices.
    pub const fn presentation(self) -> Presentation {
        self.presentation
    }
    /// Borrow original environment ordering and raw units.
    pub const fn environment(self) -> EntryEnvironmentView<'a> {
        self.environment
    }
}
macro_rules! redacted_debug {
    ($($name:ident),+ $(,)?) => { $(impl fmt::Debug for $name<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.debug_struct(stringify!($name)).finish_non_exhaustive() }
    })+ };
}
redacted_debug!(
    EntryWindowsPathView,
    EntryEnvironmentView,
    EntryArgumentsView,
    EntryConnectionView,
    EntryFrontendView,
    EntryLaunchRequestView
);
