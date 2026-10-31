// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt authorized Comfy outputs to the canvas's retained-original acquisition port.

use crate::{
    native_canvas::session::{OverviewSelection, SelectedOriginals, acquisition::SourceAcquirer},
    startup::CanvasBackend,
};
use std::{
    io,
    sync::atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_comfy::output_media::{
    ComfyOutputTransfer, ComfyViewTarget, OUTPUT_TRANSFER_BYTES,
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_image_io::{ImageSource, MAX_ENCODED_SOURCE_BYTES};
use sugarsubstitute_image_ops::Extent;
use sugarsubstitute_supervision::cache::{RuntimeCache, utc_seconds};

/// Further outputs composed with the primary into one overview document.
struct OverviewTransfers {
    extent: Extent,
    primary_cell: [f64; 4],
    cells: Vec<(ComfyOutputTransfer, [f64; 4])>,
}

/// One current backend grant with the selected Comfy media reference and either an optional
/// comparison reference or the further cells of an overview grid.
pub(super) struct ComfySource {
    transfer: ComfyOutputTransfer,
    compared: Option<ComfyOutputTransfer>,
    overview: Option<OverviewTransfers>,
    cancelled: AtomicBool,
}

impl ComfySource {
    /// Bind inert acquisition to the parent's verified peer; no network work starts here.
    pub(super) fn new(
        backend: CanvasBackend,
        target: ComfyViewTarget,
        compared: Option<ComfyViewTarget>,
    ) -> io::Result<Self> {
        Ok(Self {
            transfer: transfer(backend, target)?,
            compared: compared.map(|target| transfer(backend, target)).transpose()?,
            overview: None,
            cancelled: AtomicBool::new(false),
        })
    }

    /// Bind an overview whose first cell is the primary output; cells are physical-pixel
    /// `[left, top, width, height]` rectangles inside a document of `extent`.
    pub(super) fn overview(
        backend: CanvasBackend,
        selection: OverviewSelection<ComfyViewTarget>,
    ) -> io::Result<Self> {
        let OverviewSelection { extent, primary: (target, primary_cell), cells } = selection;
        let cells = cells
            .into_iter()
            .map(|(target, cell)| Ok((transfer(backend, target)?, cell)))
            .collect::<io::Result<_>>()?;
        Ok(Self {
            transfer: transfer(backend, target)?,
            compared: None,
            overview: Some(OverviewTransfers { extent, primary_cell, cells }),
            cancelled: AtomicBool::new(false),
        })
    }

    /// Fetch one output and retain its encoded bytes in canvas memory.
    fn fetch(
        &self,
        transfer: &ComfyOutputTransfer,
        cache: RuntimeCache,
        resident: &MemoryLease,
        context: &JobContext,
    ) -> io::Result<ImageSource> {
        self.check(context)?;
        let mut original =
            Some(transfer.fetch(cache, utc_seconds()?, context).map_err(io::Error::other)?);
        self.check(context)?;
        ImageSource::encoded(&mut original, resident).map_err(io::Error::other)
    }

    fn check(&self, context: &JobContext) -> io::Result<()> {
        if self.cancelled.load(Ordering::Acquire) || context.cancellation_requested() {
            return Err(io::ErrorKind::Interrupted.into());
        }
        if context.deadline().is_none_or(|deadline| context.now() >= deadline) {
            return Err(io::ErrorKind::TimedOut.into());
        }
        Ok(())
    }

    /// Every transfer this acquisition owns, for retirement and settlement.
    fn transfers(&self) -> impl Iterator<Item = &ComfyOutputTransfer> {
        std::iter::once(&self.transfer).chain(&self.compared).chain(
            self.overview.iter().flat_map(|overview| overview.cells.iter().map(|(cell, _)| cell)),
        )
    }
}

/// Bind one structured output reference to the verified backend peer.
fn transfer(backend: CanvasBackend, target: ComfyViewTarget) -> io::Result<ComfyOutputTransfer> {
    ComfyOutputTransfer::new(backend.origin, target, backend.peer, MAX_ENCODED_SOURCE_BYTES)
        .map_err(io::Error::other)
}

impl SourceAcquirer for ComfySource {
    fn reservation_bytes(&self) -> u64 {
        OUTPUT_TRANSFER_BYTES
    }

    fn acquire(
        &self,
        cache: RuntimeCache,
        resident: &MemoryLease,
        context: &JobContext,
    ) -> io::Result<SelectedOriginals> {
        let primary = self.fetch(&self.transfer, cache.clone(), resident, context)?;
        let compared = match &self.compared {
            Some(transfer) => Some(self.fetch(transfer, cache.clone(), resident, context)?),
            None => None,
        };
        let overview = match &self.overview {
            Some(overview) => Some(crate::native_canvas::session::OverviewOriginals {
                extent: overview.extent,
                primary_cell: overview.primary_cell,
                cells: overview
                    .cells
                    .iter()
                    .map(|(transfer, cell)| {
                        Ok((self.fetch(transfer, cache.clone(), resident, context)?, *cell))
                    })
                    .collect::<io::Result<_>>()?,
            }),
            None => None,
        };
        Ok(SelectedOriginals { primary, compared, overview })
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    fn retire(&self, context: &JobContext) -> io::Result<()> {
        let mut outcome = Ok(());
        for transfer in self.transfers() {
            let retired = transfer.retire(context).map_err(io::Error::other);
            outcome = outcome.and(retired);
        }
        outcome
    }

    fn settled(&self) -> bool {
        self.transfers().all(ComfyOutputTransfer::settled)
    }
}
