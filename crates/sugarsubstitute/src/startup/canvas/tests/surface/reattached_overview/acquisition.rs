// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Acquire overview fixtures the way the Comfy adapter does, held inside the running load job.

use super::*;
use crate::native_canvas::session::{
    OverviewOriginals, OverviewSelection, SelectedOriginals, acquisition::SourceAcquirer,
};
use std::{
    io,
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::execution::MemoryLease;
use sugarsubstitute_image_io::ImageSource;
use sugarsubstitute_supervision::cache::{Namespace, RuntimeCache, storage_key, utc_seconds};

/// Comfy's transient transfer reservation is of the same order.
const ACQUISITION_BYTES: u64 = 4 * 1024 * 1024;
/// Retained output entry name, as the Comfy adapter names PNG originals.
const ENTRY: &str = "output.png";

/// One inert overview acquisition over fixture outputs, gated after its job has started.
struct HeldAcquisition {
    gate: Arc<Latch>,
    selection: OverviewSelection<PathBuf>,
    cancelled: AtomicBool,
    settled: AtomicBool,
}

/// Build a fresh held acquisition for each overview the surface offers.
pub(super) fn held(gate: Arc<Latch>) -> crate::startup::canvas::surface::FixtureAcquisition {
    Box::new(move |selection| {
        Arc::new(HeldAcquisition {
            gate: gate.clone(),
            selection,
            cancelled: AtomicBool::new(false),
            settled: AtomicBool::new(false),
        })
    })
}

impl HeldAcquisition {
    /// Retain one output's bytes in cache custody and hand them over as an encoded original.
    fn encoded(
        &self,
        path: &Path,
        cache: &RuntimeCache,
        resident: &MemoryLease,
        context: &JobContext,
    ) -> io::Result<ImageSource> {
        if self.cancelled.load(Ordering::Acquire) || context.cancellation_requested() {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let bytes = std::fs::read(path)?;
        let length = u64::try_from(bytes.len()).map_err(io::Error::other)?;
        let key = storage_key(path.to_string_lossy().as_bytes());
        let mut entry =
            cache.clone().fresh(Namespace::ComfyOutputs, &key, length, utc_seconds()?, context)?;
        entry.write_with(ENTRY, length, context, |output| output.write_all(&bytes))?;
        entry.publish(context)?;
        let mut reader = Some(entry.into_file_reader(ENTRY, context)?);
        ImageSource::encoded(&mut reader, resident).map_err(io::Error::other)
    }

    fn originals(
        &self,
        cache: &RuntimeCache,
        resident: &MemoryLease,
        context: &JobContext,
    ) -> io::Result<SelectedOriginals> {
        let (primary, primary_cell) = &self.selection.primary;
        let cells = self
            .selection
            .cells
            .iter()
            .map(|(path, cell)| Ok((self.encoded(path, cache, resident, context)?, *cell)))
            .collect::<io::Result<_>>()?;
        Ok(SelectedOriginals {
            primary: self.encoded(primary, cache, resident, context)?,
            compared: None,
            overview: Some(OverviewOriginals {
                extent: self.selection.extent,
                primary_cell: *primary_cell,
                cells,
            }),
        })
    }
}

impl SourceAcquirer for HeldAcquisition {
    fn reservation_bytes(&self) -> u64 {
        ACQUISITION_BYTES
    }

    fn acquire(
        &self,
        cache: RuntimeCache,
        resident: &MemoryLease,
        context: &JobContext,
    ) -> io::Result<SelectedOriginals> {
        let result = self.gate.wait().and_then(|()| self.originals(&cache, resident, context));
        // Like the Comfy transfer, the acquisition retires its own native custody before return.
        self.settled.store(true, Ordering::Release);
        result
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    fn retire(&self, _: &JobContext) -> io::Result<()> {
        self.settled.store(true, Ordering::Release);
        Ok(())
    }

    fn settled(&self) -> bool {
        self.settled.load(Ordering::Acquire)
    }
}
