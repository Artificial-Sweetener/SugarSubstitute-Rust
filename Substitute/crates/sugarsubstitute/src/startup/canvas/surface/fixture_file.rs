// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serve qualification fixture files as ComfyUI input files.
use crate::native_canvas::session::{SelectedOriginals, acquisition::SourceAcquirer};
use std::{io, path::PathBuf};
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_image_io::ImageSource;
use sugarsubstitute_supervision::cache::RuntimeCache;

/// One fixture image acquired directly from its owned qualification folder.
pub(super) struct FixtureFile(PathBuf);

impl FixtureFile {
    /// Serve the image at `path`.
    pub(super) const fn new(path: PathBuf) -> Self {
        Self(path)
    }
}

impl SourceAcquirer for FixtureFile {
    fn reservation_bytes(&self) -> u64 {
        1024
    }

    fn acquire(
        &self,
        _: RuntimeCache,
        _: &MemoryLease,
        _: &JobContext,
    ) -> io::Result<SelectedOriginals> {
        Ok(SelectedOriginals::single(ImageSource::file(self.0.clone())))
    }

    fn cancel(&self) {}

    fn retire(&self, _: &JobContext) -> io::Result<()> {
        Ok(())
    }

    fn settled(&self) -> bool {
        true
    }
}
