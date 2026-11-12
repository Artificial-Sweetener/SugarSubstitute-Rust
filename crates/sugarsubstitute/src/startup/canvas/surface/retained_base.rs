// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Acquire a ComfyUI temporary file through the session's retained copy of its original.
//!
//! The first acquisition fetches the live file and retains its encoded original; every later
//! one, including after a restart once ComfyUI has deleted its temporary files, uses the
//! retained original the user authored on. See [`crate::input_documents::bases`].
use crate::input_documents::bases::RetainedBases;
use crate::native_canvas::session::{SelectedOriginals, acquisition::SourceAcquirer};
use std::{io, sync::Arc};
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_image_io::{ImageSource, MAX_ENCODED_SOURCE_BYTES};
use sugarsubstitute_supervision::cache::RuntimeCache;

/// One temporary file's acquisition, preferring its retained original.
pub(super) struct RetainedBase {
    reference: Arc<str>,
    live: Arc<dyn SourceAcquirer>,
    bases: RetainedBases,
}

impl RetainedBase {
    /// Acquire `reference` from `bases`, or from `live` and retain it there.
    pub(super) fn new(
        reference: Arc<str>,
        live: Arc<dyn SourceAcquirer>,
        bases: RetainedBases,
    ) -> Self {
        Self { reference, live, bases }
    }
}

impl SourceAcquirer for RetainedBase {
    /// The live acquisition's own working bytes. Retained and copied originals are charged to
    /// the resident lease as they are read, like every other acquired original.
    fn reservation_bytes(&self) -> u64 {
        self.live.reservation_bytes()
    }

    fn acquire(
        &self,
        cache: RuntimeCache,
        resident: &MemoryLease,
        context: &JobContext,
    ) -> io::Result<SelectedOriginals> {
        let retained = self
            .bases
            .find(&self.reference, MAX_ENCODED_SOURCE_BYTES, resident, context)
            .map_err(io::Error::other)?;
        if let Some(original) = retained {
            let primary =
                ImageSource::memory(&mut Some(original), resident).map_err(io::Error::other)?;
            return Ok(SelectedOriginals::single(primary));
        }
        let originals = self.live.acquire(cache, resident, context)?;
        let (bytes, _claim) = originals
            .primary
            .encoded_bytes(MAX_ENCODED_SOURCE_BYTES, resident, context)
            .map_err(io::Error::other)?;
        self.bases.retain(&self.reference, &bytes, resident, context).map_err(io::Error::other)?;
        Ok(originals)
    }

    fn cancel(&self) {
        self.live.cancel();
    }

    fn retire(&self, context: &JobContext) -> io::Result<()> {
        self.live.retire(context)
    }

    fn settled(&self) -> bool {
        self.live.settled()
    }
}
