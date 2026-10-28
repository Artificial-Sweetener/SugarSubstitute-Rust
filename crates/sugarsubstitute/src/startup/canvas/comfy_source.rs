// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt one authorized Comfy output to the canvas's retained-original acquisition port.

use crate::{native_canvas::session::acquisition::SourceAcquirer, startup::CanvasBackend};
use std::{
    io,
    sync::atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_comfy::output_media::{
    ComfyOutputTransfer, ComfyViewTarget, OUTPUT_TRANSFER_BYTES,
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_image_io::{ImageSource, MAX_ENCODED_SOURCE_BYTES};
use sugarsubstitute_supervision::cache::{RuntimeCache, utc_seconds};

/// One current backend grant and one structured, untrusted Comfy media reference.
pub(super) struct ComfySource {
    transfer: ComfyOutputTransfer,
    cancelled: AtomicBool,
}

impl ComfySource {
    /// Bind inert acquisition to the parent's verified peer; no network work starts here.
    pub(super) fn new(backend: CanvasBackend, target: ComfyViewTarget) -> io::Result<Self> {
        let transfer = ComfyOutputTransfer::new(
            backend.origin,
            target,
            backend.peer,
            MAX_ENCODED_SOURCE_BYTES,
        )
        .map_err(io::Error::other)?;
        Ok(Self { transfer, cancelled: AtomicBool::new(false) })
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
    ) -> io::Result<ImageSource> {
        self.check(context)?;
        let mut original =
            Some(self.transfer.fetch(cache, utc_seconds()?, context).map_err(io::Error::other)?);
        self.check(context)?;
        ImageSource::encoded(&mut original, resident).map_err(io::Error::other)
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    fn retire(&self, context: &JobContext) -> io::Result<()> {
        self.transfer.retire(context).map_err(io::Error::other)
    }

    fn settled(&self) -> bool {
        self.transfer.settled()
    }
}
