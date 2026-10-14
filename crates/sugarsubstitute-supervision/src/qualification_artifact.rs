// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Persist one bounded genuine preview through pinned ancestry and checked native file retirement.
//!
//! The composing registered host owns the development intent and validates its D destination.
//! This adapter accepts only create-only PNG output, never generic path operations or cleanup.
//! All methods execute in the original finite central job; no native I/O runs on a UI thread.

mod native;
mod validation;
use validation::{check, copy_parent, validate, validate_png};
#[cfg(test)]
mod tests;
use crate::{
    ClaimError,
    native_claims::files::{Budget, Handle},
};
use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use sugarsubstitute_contracts::{
    execution::{JobContext, MemoryClaim, Moment},
    startup::MainIdentity,
};
use sugarsubstitute_execution::PreviewArmingSeal;
use windows::Win32::Foundation::HANDLE;

/// Production custody always owns the genuine Execution-derived purpose seal.
struct Authority {
    seal: Option<PreviewArmingSeal>,
    #[cfg(test)]
    filesystem_boundary: Option<(PathBuf, sugarsubstitute_contracts::execution::MemoryClaim)>,
}
impl Authority {
    fn destination(&self) -> Result<&Path, ClaimError> {
        if let Some(seal) = &self.seal {
            return Ok(seal.destination());
        }
        #[cfg(test)]
        if let Some((path, _)) = &self.filesystem_boundary {
            return Ok(path);
        }
        Err(ClaimError::invalid("preview_artifact_authority"))
    }
    fn check(&self, context: &JobContext, identity: MainIdentity) -> Result<(), ClaimError> {
        if let Some(seal) = &self.seal {
            return seal
                .check(context, identity)
                .map_err(|cause| ClaimError::new("validate_preview_source", cause));
        }
        #[cfg(test)]
        if self.filesystem_boundary.is_some() {
            return Ok(());
        }
        Err(ClaimError::invalid("preview_artifact_authority"))
    }
}

/// Fixed authored storage including bounded paths, native output cells and validation scratch.
pub const STORAGE_BYTES: u64 = 32 * 1024;
/// One PNG output plus at most thirty-two pinned existing ancestry directories.
pub const OUTPUT_SLOTS: usize = 33;
/// Maximum admitted encoded preview bytes; this adapter never duplicates the image.
pub const IMAGE_BYTES: usize = 2 * 1024 * 1024;

/// Actual finite job's once-created artifact owner, retained even after uncertain native closure.
pub struct PreviewArtifact {
    authority: Authority,
    identity: MainIdentity,
    viewport: (u32, u32),
    until: Moment,
    context: JobContext,
    handles: Vec<Handle>,
    directories: usize,
    output: Option<usize>,
    initialized: bool,
    written: bool,
    activated: bool,
    memory: Arc<MemoryClaim>,
}

impl PreviewArtifact {
    /// Preclaim fixed path/output storage before any filesystem effect.
    ///
    /// The move-only seal binds the authenticated original capture source, both existing scopes,
    /// actual ordinary job, original campaign and owned D destination. Geometry is actual
    /// renderer data and cannot grant file authority. Rejection consumes no filesystem output.
    ///
    /// # Errors
    /// Rejects expired/cancelled/nonfinite jobs, nonlocal/non-PNG/malformed or oversized paths,
    /// excessive directory depth, invalid pixel extent and insufficient actual job credit.
    pub fn new(
        seal: PreviewArmingSeal,
        identity: MainIdentity,
        viewport: (u32, u32),
        context: &JobContext,
    ) -> Result<Self, ClaimError> {
        let until = seal.original_deadline();
        Self::prepare(
            Authority {
                seal: Some(seal),
                #[cfg(test)]
                filesystem_boundary: None,
            },
            identity,
            viewport,
            until,
            context,
        )
    }

    fn prepare(
        authority: Authority,
        identity: MainIdentity,
        viewport: (u32, u32),
        until: Moment,
        context: &JobContext,
    ) -> Result<Self, ClaimError> {
        authority.check(context, identity)?;
        check(context, until)?;
        validate(authority.destination()?, viewport)?;
        let memory = Arc::new(
            context
                .memory()
                .claim(STORAGE_BYTES)
                .map_err(|cause| ClaimError::new("claim_preview_artifact", cause))?,
        );
        let mut handles = Vec::new();
        handles
            .try_reserve_exact(OUTPUT_SLOTS)
            .map_err(|cause| ClaimError::new("allocate_preview_outputs", cause))?;
        if handles.capacity() != OUTPUT_SLOTS
            || size_of::<Self>() + handles.capacity() * size_of::<Handle>() + native::SCRATCH_BYTES
                > STORAGE_BYTES as usize
        {
            return Err(ClaimError::invalid("preview_artifact_layout"));
        }
        Ok(Self {
            authority,
            identity,
            viewport,
            until,
            context: context.clone(),
            handles,
            directories: 0,
            output: None,
            initialized: false,
            written: false,
            activated: false,
            memory,
        })
    }

    /// Fake only the separate source-authorization boundary in filesystem-owner tests.
    #[cfg(test)]
    fn fixture_new(
        destination: &Path,
        identity: MainIdentity,
        viewport: (u32, u32),
        until: Moment,
        context: &JobContext,
    ) -> Result<Self, ClaimError> {
        let storage = context
            .memory()
            .claim((destination.as_os_str().len() + size_of::<PathBuf>()) as u64)
            .map_err(|cause| ClaimError::new("claim_fixture_source", cause))?;
        let mut path = PathBuf::new();
        path.try_reserve_exact(destination.as_os_str().len())
            .map_err(|cause| ClaimError::new("allocate_fixture_destination", cause))?;
        if path.capacity() > destination.as_os_str().len() {
            return Err(ClaimError::invalid("fixture_destination_capacity"));
        }
        path.push(destination);
        Self::prepare(
            Authority { seal: None, filesystem_boundary: Some((path, storage)) },
            identity,
            viewport,
            until,
            context,
        )
    }

    fn check(&self) -> Result<(), ClaimError> {
        self.authority.check(&self.context, self.identity)?;
        check(&self.context, self.until)
    }

    /// Pin each actual existing ancestor and create the one destination exclusively.
    ///
    /// # Errors
    /// Retains adopted handles/first filesystem cause on partial failure; existing output,
    /// reparse ancestry, foreign resolved volume, directory mutation or late cancellation rejects.
    pub fn initialize(&mut self) -> Result<(), ClaimError> {
        if self.initialized || !self.handles.is_empty() {
            return Err(ClaimError::invalid("preview_artifact_repeated_initialization"));
        }
        self.initialized = true;
        let mut ancestor = PathBuf::new();
        ancestor
            .try_reserve_exact(self.authority.destination()?.as_os_str().len())
            .map_err(|cause| ClaimError::new("allocate_preview_ancestor", cause))?;
        if ancestor.capacity() > 4096 {
            return Err(ClaimError::invalid("preview_artifact_path_capacity"));
        }
        let parent = copy_parent(self.authority.destination()?)?;
        for component in parent.components() {
            ancestor.push(component.as_os_str());
            if matches!(component, Component::Prefix(_)) {
                continue;
            }
            self.check()?;
            let index = self.reserve()?;
            native::open_directory(&ancestor, &mut self.handles[index])?;
            native::validate_directory(&mut self.handles[index], &ancestor)?;
            self.directories += 1;
        }
        self.check()?;
        self.validate_ancestry()?;
        let index = self.reserve()?;
        self.output = Some(index);
        native::create_png(self.authority.destination()?, &mut self.handles[index])?;
        self.check()?;
        native::validate_output(&mut self.handles[index], self.authority.destination()?)?;
        Ok(())
    }

    /// Write the original borrowed PNG in bounded chunks without copying its allocation.
    ///
    /// # Errors
    /// Rejects nonnative-sized PNG headers, duplicate writes, missing actual output and
    /// any original cancellation/deadline/I/O failure. Partial output remains incomplete.
    pub fn write_png(&mut self, bytes: &[u8]) -> Result<(), ClaimError> {
        self.check()?;
        validate_png(bytes, self.viewport)?;
        if self.written || !self.initialized {
            return Err(ClaimError::invalid("preview_artifact_write_state"));
        }
        let index = self.output.ok_or_else(|| ClaimError::invalid("preview_artifact_output"))?;
        for chunk in bytes.chunks(16 * 1024) {
            self.check()?;
            native::write(&mut self.handles[index], chunk)?;
            self.check()?;
        }
        self.written = true;
        Ok(())
    }

    /// Flush and revalidate the pinned namespace/output before recording successful activation.
    ///
    /// # Errors
    /// Original cutoff/cancellation, missing written output, changed ancestry, native flush or
    /// final output validation failure prevents activation. This does not close any handle.
    pub fn activate(&mut self) -> Result<(), ClaimError> {
        self.check()?;
        if !self.written || self.activated {
            return Err(ClaimError::invalid("preview_artifact_activation_state"));
        }
        let index = self.output.ok_or_else(|| ClaimError::invalid("preview_artifact_output"))?;
        self.validate_ancestry()?;
        native::validate_output(&mut self.handles[index], self.authority.destination()?)?;
        native::flush(&mut self.handles[index])?;
        self.check()?;
        self.validate_ancestry()?;
        native::validate_output(&mut self.handles[index], self.authority.destination()?)?;
        self.activated = true;
        Ok(())
    }

    /// Close every actually adopted output in reverse order, including cancelled/failed work.
    ///
    /// # Errors
    /// Native close failure keeps the exact output cell and original shared claim. No Drop,
    /// deadline, requested flush or successful activation is proof that handles retired.
    pub fn retire(&mut self) -> Result<(), ClaimError> {
        self.retire_with(Handle::close)
    }

    /// Attempt every owned cell; a rejected native boundary leaves its original cell armed.
    fn retire_with(
        &mut self,
        mut close: impl FnMut(&mut Handle) -> Result<(), ClaimError>,
    ) -> Result<(), ClaimError> {
        let mut first = None;
        for handle in self.handles.iter_mut().rev() {
            if let Err(cause) = close(handle) {
                if first.is_none() {
                    first = Some(cause);
                } else {
                    tracing::error!(operation="close_preview_artifact", cause=%cause,
                        "Additional native close failure retained by its actual output cell");
                }
            }
        }
        first.map_or(Ok(()), Err)
    }

    /// Observe actual known output custody independently of worker terminal status.
    pub fn retired(&self) -> bool {
        self.handles.iter().all(|handle| !handle.is_armed())
    }
    /// Return successful checked activation; complete acceptance also requires actual retirement.
    pub const fn activated(&self) -> bool {
        self.activated
    }
    /// Return the same original application's main identity without granting filesystem authority.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }

    fn reserve(&mut self) -> Result<usize, ClaimError> {
        if self.handles.len() == OUTPUT_SLOTS {
            return Err(ClaimError::invalid("preview_artifact_output_capacity"));
        }
        let index = self.handles.len();
        self.handles.push(Handle::new(HANDLE::default(), Budget::Protected(self.memory.clone())));
        Ok(index)
    }

    fn validate_ancestry(&mut self) -> Result<(), ClaimError> {
        let mut ancestor = PathBuf::new();
        ancestor
            .try_reserve_exact(self.authority.destination()?.as_os_str().len())
            .map_err(|cause| ClaimError::new("allocate_preview_ancestor", cause))?;
        if ancestor.capacity() > 4096 {
            return Err(ClaimError::invalid("preview_artifact_path_capacity"));
        }
        let parent = copy_parent(self.authority.destination()?)?;
        let mut index = 0;
        for component in parent.components() {
            ancestor.push(component.as_os_str());
            if matches!(component, Component::Prefix(_)) {
                continue;
            }
            self.check()?;
            native::validate_directory(&mut self.handles[index], &ancestor)?;
            index += 1;
        }
        if index != self.directories {
            return Err(ClaimError::invalid("preview_artifact_ancestry_changed"));
        }
        Ok(())
    }
}
