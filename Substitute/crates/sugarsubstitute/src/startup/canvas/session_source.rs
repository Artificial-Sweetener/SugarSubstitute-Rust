// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Acquire host-owned retained finals through the existing native image/cache lifetime.
mod original;
use crate::native_canvas::session::{
    OverviewOriginals, OverviewSelection, SelectedOriginals, acquisition::SourceAcquirer,
};
use original::Original;
use std::{
    io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use sugarsubstitute_comfy::output_media::{ComfyViewTarget, OutputArtifactIdentity};
use sugarsubstitute_contracts::{
    desktop_session::wire::SessionAsset,
    execution::{JobContext, MemoryClaim, MemoryLease},
};
use sugarsubstitute_image_io::ImageSource;
use sugarsubstitute_supervision::cache::RuntimeCache;

/// Exact root-verified originals in the same order as one current native request. The root
/// matches output UUID, descriptor and the independently adopted runtime/generation first.
pub(in crate::startup) enum OriginalSpec {
    /// Known original whose current runtime has no native acquisition admission.
    Unavailable,
    Retained {
        path: PathBuf,
        asset: SessionAsset,
    },
    Backend {
        backend: super::super::CanvasBackend,
        target: ComfyViewTarget,
        expected: Option<OutputArtifactIdentity>,
    },
}
/// Inert request-scoped adapter data. It grants no path selection from canvas messages.
pub(in crate::startup) struct SessionSource {
    originals: Vec<OriginalSpec>,
    memory: Arc<MemoryClaim>,
}
impl SessionSource {
    pub(in crate::startup) fn new(originals: Vec<OriginalSpec>, memory: MemoryClaim) -> Self {
        Self { originals, memory: Arc::new(memory) }
    }
    /// Keep a bounded selection's correlation charged after its request is answered.
    pub(super) fn correlation_memory(&self) -> Arc<MemoryClaim> {
        self.memory.clone()
    }
    /// An explicit refusal also fences retained pixels before any acquisition is attempted.
    pub(super) fn unavailable(&self) -> bool {
        self.originals.iter().any(|original| matches!(original, OriginalSpec::Unavailable))
    }
    /// Only wholly live sources may reuse the existing backend descriptor retention key.
    pub(super) fn live_retention(&self) -> bool {
        self.originals
            .iter()
            .all(|original| matches!(original, OriginalSpec::Backend { expected: None, .. }))
    }
    /// Expected original hashes already admitted by the root's current receipt classifier.
    pub(super) fn original_hashes(&self) -> Option<Vec<[u8; 32]>> {
        self.originals
            .iter()
            .map(|original| match original {
                OriginalSpec::Retained { asset, .. } => {
                    OutputArtifactIdentity::new(&asset.digest, asset.bytes)
                        .ok()
                        .map(|identity| *identity.digest())
                }
                OriginalSpec::Backend { expected: Some(expected), .. } => Some(*expected.digest()),
                OriginalSpec::Backend { expected: None, .. } | OriginalSpec::Unavailable => None,
            })
            .collect()
    }
    pub(super) fn retention_key(&self) -> Option<crate::native_canvas::session::RetentionKey> {
        if self.originals.is_empty() || self.originals.len() > 2 {
            return None;
        }
        let mut key = format!("session-originals/{}", self.originals.len());
        for original in &self.originals {
            let (digest, bytes) = match original {
                OriginalSpec::Retained { asset, .. } => (asset.digest.clone(), asset.bytes),
                OriginalSpec::Backend { expected: Some(expected), .. } => (
                    expected.digest().iter().map(|byte| format!("{byte:02x}")).collect(),
                    expected.bytes(),
                ),
                OriginalSpec::Backend { expected: None, .. } | OriginalSpec::Unavailable => {
                    return None;
                }
            };
            key.push_str(&format!("/{digest}/{bytes}"));
        }
        Some(crate::native_canvas::session::RetentionKey::new(&key))
    }
    pub(super) fn single(self) -> io::Result<Arc<dyn SourceAcquirer>> {
        if self.originals.is_empty() || self.originals.len() > 2 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(Arc::new(Acquisition {
            originals: self.originals.into_iter().map(Original::new).collect::<io::Result<_>>()?,
            overview: None,
            cancelled: AtomicBool::new(false),
            _memory: self.memory,
        }))
    }
    pub(super) fn overview(
        self,
        selection: OverviewSelection<ComfyViewTarget>,
    ) -> io::Result<Arc<dyn SourceAcquirer>> {
        if self.originals.len() != selection.cells.len() + 1 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let geometry = Geometry {
            extent: selection.extent,
            primary: selection.primary.1,
            cells: selection.cells.into_iter().map(|(_, rect)| rect).collect(),
        };
        Ok(Arc::new(Acquisition {
            originals: self.originals.into_iter().map(Original::new).collect::<io::Result<_>>()?,
            overview: Some(geometry),
            cancelled: AtomicBool::new(false),
            _memory: self.memory,
        }))
    }
}
struct Geometry {
    extent: sugarsubstitute_image_ops::Extent,
    primary: [f64; 4],
    cells: Vec<[f64; 4]>,
}
struct Acquisition {
    originals: Vec<Original>,
    overview: Option<Geometry>,
    cancelled: AtomicBool,
    _memory: Arc<MemoryClaim>,
}
impl Acquisition {
    fn controls(&self, context: &JobContext) -> io::Result<()> {
        if self.cancelled.load(Ordering::Acquire) || context.cancellation_requested() {
            return Err(io::ErrorKind::Interrupted.into());
        }
        if context.deadline().is_none_or(|until| context.now() >= until) {
            return Err(io::ErrorKind::TimedOut.into());
        }
        Ok(())
    }
}
impl SourceAcquirer for Acquisition {
    fn reservation_bytes(&self) -> u64 {
        sugarsubstitute_comfy::output_media::OUTPUT_TRANSFER_BYTES
    }
    fn acquire(
        &self,
        cache: RuntimeCache,
        resident: &MemoryLease,
        context: &JobContext,
    ) -> io::Result<SelectedOriginals> {
        self.controls(context)?;
        let fetch = |index: usize| -> io::Result<ImageSource> {
            self.controls(context)?;
            self.originals
                .get(index)
                .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?
                .fetch(cache.clone(), resident, context)
        };
        let primary = fetch(0)?;
        match &self.overview {
            Some(geometry) => Ok(SelectedOriginals {
                primary,
                compared: None,
                overview: Some(OverviewOriginals {
                    extent: geometry.extent,
                    primary_cell: geometry.primary,
                    cells: geometry
                        .cells
                        .iter()
                        .enumerate()
                        .map(|(index, rect)| Ok((fetch(index + 1)?, *rect)))
                        .collect::<io::Result<_>>()?,
                }),
            }),
            None => Ok(SelectedOriginals {
                primary,
                compared: (self.originals.len() == 2).then(|| fetch(1)).transpose()?,
                overview: None,
            }),
        }
    }
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    fn retire(&self, context: &JobContext) -> io::Result<()> {
        let mut outcome = Ok(());
        for original in &self.originals {
            let retired = original.retire(context);
            outcome = outcome.and(retired);
        }
        outcome
    }
    fn settled(&self) -> bool {
        self.originals.iter().all(Original::settled)
    }
}
