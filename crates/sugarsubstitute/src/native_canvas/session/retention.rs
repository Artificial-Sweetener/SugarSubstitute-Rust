// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain recently shown output documents so reselecting one presents its finished frame at once.
//!
//! A retained document keeps its scene (authored document, pinned source reader and original),
//! its camera and its full-quality detail and whole-document fallback backings, whose memory
//! claims stay funded by the session's retained-document allowance. Display custody (the
//! projection, its buffers and acknowledged evidence) always stays with the current document.
//! Eviction, forgetting and close hand documents to the existing retirement worker.

use super::*;
use std::collections::VecDeque;

/// Opaque host identity of one retainable output selection.
///
/// The host derives it from its backend and output reference; the session only compares it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RetentionKey(Arc<str>);

impl RetentionKey {
    /// Identify one output by its host-qualified reference.
    pub(crate) fn new(identity: &str) -> Self {
        Self(Arc::from(identity))
    }
}

/// One parked document and the backing bytes its ledger entry charges.
struct Entry {
    key: RetentionKey,
    document: Box<document::DocumentState>,
    bytes: u64,
}

/// Documents and backings leaving retention; disposed of only on the retirement worker.
#[derive(Default)]
pub(super) struct Retiring {
    documents: Vec<Box<document::DocumentState>>,
    surfaces: Vec<Arc<CpuSurface>>,
    gpu_surfaces: Vec<GpuSurface>,
}

impl Retiring {
    /// Whether nothing awaits disposal.
    fn is_empty(&self) -> bool {
        self.documents.is_empty() && self.surfaces.is_empty() && self.gpu_surfaces.is_empty()
    }
}

/// Parked documents ordered from least to most recently shown, bounded by a count and by a byte
/// ledger of their backings (four bytes per backing pixel, as the canvas budget funds them).
#[derive(Default)]
pub(super) struct RetainedDocuments {
    entries: VecDeque<Entry>,
    bytes: u64,
    retiring: Retiring,
}

impl RetainedDocuments {
    /// Whether a document for `key` is parked.
    pub(super) fn contains(&self, key: &RetentionKey) -> bool {
        self.entries.iter().any(|entry| entry.key == *key)
    }

    /// Parked document count.
    #[cfg(test)]
    pub(super) fn count(&self) -> usize {
        self.entries.len()
    }

    /// Backing bytes the parked documents charge to the retained-document allowance.
    #[cfg(test)]
    pub(super) fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Whether evicted or forgotten documents still await the retirement worker.
    #[cfg(test)]
    pub(super) fn retiring_pending(&self) -> bool {
        !self.retiring.is_empty()
    }

    /// Withdraw the parked document for `key`, releasing its ledger charge.
    fn take(&mut self, key: &RetentionKey) -> Option<Box<document::DocumentState>> {
        let index = self.entries.iter().position(|entry| entry.key == *key)?;
        let entry = self.entries.remove(index)?;
        self.bytes = self.bytes.saturating_sub(entry.bytes);
        Some(entry.document)
    }

    /// Park `document` as the most recently shown, then evict the least recently shown documents
    /// while more than `count` remain or their backings exceed `allowance` bytes.
    fn park(
        &mut self,
        key: RetentionKey,
        document: Box<document::DocumentState>,
        count: usize,
        allowance: u64,
    ) {
        if let Some(previous) = self.take(&key) {
            self.retiring.documents.push(previous);
        }
        let bytes = document.retained_bytes();
        self.bytes = self.bytes.saturating_add(bytes);
        self.entries.push_back(Entry { key, document, bytes });
        while self.entries.len() > count || self.bytes > allowance {
            let Some(evicted) = self.entries.pop_front() else {
                break;
            };
            self.bytes = self.bytes.saturating_sub(evicted.bytes);
            tracing::debug!(
                operation = "canvas_retained_eviction",
                evicted_bytes = evicted.bytes,
                retained_documents = self.entries.len(),
                retained_bytes = self.bytes,
                "Evicted the least recently shown retained canvas document"
            );
            self.retiring.documents.push(evicted.document);
        }
    }

    /// Move every parked document to retirement.
    fn forget(&mut self) {
        self.retiring.documents.extend(self.entries.drain(..).map(|entry| entry.document));
        self.bytes = 0;
    }

    /// Transfer everything awaiting retirement, if anything is.
    fn take_retiring(&mut self) -> Option<Retiring> {
        (!self.retiring.is_empty()).then(|| std::mem::take(&mut self.retiring))
    }

    /// Transfer every parked document and pending retirement for close.
    pub(super) fn take_all(&mut self) -> Option<Retiring> {
        self.forget();
        self.take_retiring()
    }

    /// Whether parked or retiring documents still hold worker-disposed resources.
    pub(super) fn has_resources(&self) -> bool {
        !self.entries.is_empty() || !self.retiring.is_empty()
    }
}

impl document::DocumentState {
    /// Whether this document can be shown again without preparation: a keyed single original
    /// whose full-quality detail and whole-document fallback belong to its current source.
    fn retainable(&self) -> bool {
        let Some(scene) = &self.scene else {
            return false;
        };
        if self.retention.is_none()
            || self.cpu_only
            || scene.source.is_none()
            || scene.compared.is_some()
            || !scene.placed.is_empty()
            || (self.front.is_none() && self.gpu_front.is_none())
        {
            return false;
        }
        let snapshot = scene.document.snapshot();
        let identity = (snapshot.identity(), snapshot.revision());
        let current = |shown: &Presented| {
            shown.document == identity
                && shown.source_revision == scene.source_revision
                && shown.comparison.is_none()
        };
        self.presented.as_ref().is_some_and(|shown| {
            current(shown)
                && shown.tier == BackingTier::Detail
                && !shown.reconstruction.is_preview()
        }) && self.fallback_presented.as_ref().is_some_and(current)
    }

    /// Backing bytes charged to the retained-document ledger.
    fn retained_bytes(&self) -> u64 {
        let cpu = [&self.front, &self.fallback_front, &self.navigation_front]
            .into_iter()
            .flatten()
            .map(|surface| surface.extent().pixels());
        let gpu = [&self.gpu_front, &self.fallback_gpu_front, &self.navigation_gpu_front]
            .into_iter()
            .flatten()
            .map(|surface| surface.extent().pixels());
        cpu.chain(gpu).fold(0, |total: u64, pixels| total.saturating_add(pixels.saturating_mul(4)))
    }

    /// Release the navigation tier, which only serves motion and is rebuilt on demand.
    fn trim(&mut self, retiring: &mut Retiring) {
        retiring.surfaces.extend(self.navigation_front.take());
        retiring.gpu_surfaces.extend(self.navigation_gpu_front.take());
        self.navigation_presented = None;
    }

    /// Take over the outgoing document's display custody: the acknowledged projection, its
    /// reusable buffers and their evidence, which describe the screen rather than a document.
    /// Neither document may hold a frame awaiting native acknowledgement.
    fn adopt_display(&mut self, outgoing: &mut Self) {
        self.cpu_projection = outgoing.cpu_projection.take();
        self.display_buffers = std::mem::take(&mut outgoing.display_buffers);
        self.display_buffer_extent = outgoing.display_buffer_extent.take();
        self.display_source = outgoing.display_source.take();
        self.display_view = outgoing.display_view.take();
        self.display_evidence = outgoing.display_evidence.take();
        self.native_presented = outgoing.native_presented.take();
        self.native_verified = std::mem::take(&mut outgoing.native_verified);
        self.projection_damage = outgoing.projection_damage.take();
        self.projection_base = outgoing.projection_base.take();
    }

    /// Label the scene and the backings prepared for its current source as `source_revision`.
    /// Backings of any older source stay stale.
    fn rebind(&mut self, source_revision: u64) {
        let Some(scene) = &mut self.scene else {
            return;
        };
        let previous = std::mem::replace(&mut scene.source_revision, source_revision);
        for shown in
            [&mut self.presented, &mut self.fallback_presented, &mut self.navigation_presented]
                .into_iter()
                .flatten()
                .filter(|shown| shown.source_revision == previous)
        {
            shown.source_revision = source_revision;
        }
        self.initial_preview = false;
    }
}

impl Session {
    /// Tag the latest offered source with the host identity of the output it shows, so its
    /// document can be retained once a newer output replaces it. Comparison and overview
    /// selections stay untagged; they are never retained.
    pub(crate) fn retain_selection(&mut self, key: RetentionKey) {
        let selection = self.selection;
        if let Some(open) = self.pending_open.as_mut().filter(|open| open.intent == selection) {
            open.retention = Some(key);
        }
    }

    /// Select a retained output again without acquiring or preparing it.
    ///
    /// On `true` this is the current selection: the retained document becomes current with its
    /// camera once earlier document work has settled, and its finished frame is displayed
    /// directly. Returns `false` without any change when no document for `key` is retained, or
    /// while closing, GPU-window presentation, an attachment transaction, preview custody or a
    /// pending source retirement prevents reuse; the caller then offers the source as usual.
    ///
    /// # Errors
    /// Rejects exhausted source identities and inconsistent pending CPU custody.
    pub(crate) fn reselect_retained(&mut self, key: &RetentionKey) -> Result<bool, Box<dyn Error>> {
        if self.closing
            || self.presentation.gpu_window().is_some()
            || self.resources.is_none()
            || self.document_suspended
            || self.document_transaction.is_some()
            || self.preview_pending()
        {
            return Ok(false);
        }
        let current = self.document.retention.as_ref() == Some(key)
            && self.document.scene.as_ref().is_some_and(|scene| scene.source.is_some());
        if !current && !self.retained.contains(key) {
            return Ok(false);
        }
        let Some(intent) = self.supersede_pending_source()? else {
            return Ok(false);
        };
        self.pending_restore = Some((key.clone(), intent));
        Ok(true)
    }

    /// Retire every retained document, as when the outputs' backend changes. A pending
    /// reselection of the current document itself still completes.
    pub(crate) fn forget_retained(&mut self) {
        self.retained.forget();
        if self
            .pending_restore
            .as_ref()
            .is_some_and(|(key, _)| self.document.retention.as_ref() != Some(key))
        {
            self.pending_restore = None;
        }
    }

    /// Whether document custody may move: no job or native frame references the current document.
    fn document_parkable(&self) -> bool {
        #[cfg(windows)]
        let composition = self.composition_pending.is_some();
        #[cfg(not(windows))]
        let composition = false;
        self.rendering.is_none()
            && self.displaying.is_none()
            && self.presenting.is_none()
            && self.awaiting_gpu.is_none()
            && !composition
            && self.document.cpu_pending.is_none()
            && self.document.pending_cpu_evidence.is_none()
            && self.document.pending_gpu_evidence.is_none()
    }

    /// Whether this session may park `document` for reselection.
    fn retains(&self, document: &document::DocumentState) -> bool {
        self.presentation.gpu_window().is_none() && document.retainable()
    }

    /// Hold a finished load while the retainable outgoing document still has display work, and
    /// fence that work so the document can be parked on a later pass.
    ///
    /// # Errors
    /// Preserves CPU and composition frame withdrawal failures.
    pub(super) fn parking_awaits_document_work(&mut self) -> Result<bool, Box<dyn Error>> {
        let finished = self.loading.as_ref().is_some_and(|loading| {
            loading.expected.is_none()
                && matches!(loading.job.status().phase, JobPhase::Terminal(_))
        });
        if !finished || !self.retains(&self.document) || self.document_parkable() {
            return Ok(false);
        }
        self.fence_document_work()?;
        Ok(true)
    }

    /// Cancel the current document's rendering and display work and withdraw its unpresented
    /// frames, so its custody can move once their cancellations are observed.
    ///
    /// # Errors
    /// Preserves CPU and composition frame withdrawal failures.
    fn fence_document_work(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(displaying) = &self.displaying {
            displaying.job.cancel();
        }
        if let Some(rendering) = &mut self.rendering {
            rendering.superseded = true;
            rendering.job.cancel();
        }
        let now = self.navigation_time();
        self.abandon_cpu_frame(now)?;
        #[cfg(windows)]
        self.abandon_composition_frame(now)?;
        Ok(())
    }

    /// Prepare the current document to receive a newly loaded output tagged `retention`.
    /// A retainable outgoing document is parked with its camera and backings, and the new
    /// output starts in a fresh document that keeps the current display custody; otherwise the
    /// new scene replaces the current document's scene in place.
    ///
    /// # Errors
    /// Preserves viewport and refresh-rate validation failures.
    pub(super) fn adopt_document_for(
        &mut self,
        retention: Option<RetentionKey>,
    ) -> Result<(), Box<dyn Error>> {
        if self.retains(&self.document) && self.document_parkable() {
            let surface = self.document.tools.navigation().viewport().surface();
            let mut fresh = Box::new(document::DocumentState::new(Viewport::new(surface)?));
            fresh.tools.set_refresh_rate(self.display_refresh)?;
            let mut outgoing = std::mem::replace(&mut self.document, fresh);
            self.document.adopt_display(&mut outgoing);
            self.park(outgoing);
        }
        self.document.retention = retention;
        Ok(())
    }

    /// Park a retainable document as the most recently shown, or retire it.
    fn park(&mut self, mut document: Box<document::DocumentState>) {
        match document.retention.clone() {
            Some(key) if !self.closing && self.retains(&document) => {
                document.trim(&mut self.retained.retiring);
                self.retained.park(
                    key,
                    document,
                    budget::RETAINED_DOCUMENTS,
                    self.config.retained_document_bytes,
                );
            }
            _ => self.retained.retiring.documents.push(document),
        }
    }

    /// Make a reselected retained document current once earlier document work has settled.
    /// Returns `true` while ordinary document admission must keep waiting for that settlement.
    ///
    /// # Errors
    /// Preserves camera geometry, refresh-rate and navigation clock failures.
    pub(super) fn advance_retained_restore(&mut self) -> Result<bool, Box<dyn Error>> {
        let Some((_, intent)) = &self.pending_restore else {
            return Ok(false);
        };
        if *intent != self.selection {
            self.pending_restore = None;
            return Ok(false);
        }
        if self.loading.is_some()
            || self.pending_open.is_some()
            || self.document_suspended
            || self.document_transaction.is_some()
            || self.preview_pending()
        {
            return Ok(true);
        }
        if !self.document_parkable() {
            // The outgoing document's remaining work can no longer be shown.
            self.fence_document_work()?;
            return Ok(true);
        }
        let Some((key, intent)) = self.pending_restore.take() else {
            return Ok(false);
        };
        self.restore_retained(key, intent)
    }

    /// Swap the retained document for `key` in as selection `intent`, parking the outgoing one,
    /// or relabel the current document when it already shows `key`. The restored camera is kept,
    /// fitted to the current physical extent, and one fresh display of the retained backings
    /// proves the selection.
    fn restore_retained(&mut self, key: RetentionKey, intent: u64) -> Result<bool, Box<dyn Error>> {
        let now = self.navigation_time();
        let extent = self.document.tools.navigation().viewport().surface();
        if self.document.retention.as_ref() != Some(&key) {
            let Some(incoming) = self.retained.take(&key) else {
                self.fail(WorkError::invalid("The retained canvas document is unavailable"));
                return Ok(false);
            };
            let mut outgoing = std::mem::replace(&mut self.document, incoming);
            self.document.adopt_display(&mut outgoing);
            self.park(outgoing);
        }
        let document = &mut *self.document;
        document.rebind(intent);
        document.tools.set_refresh_rate(self.display_refresh)?;
        document.tools.resize(extent, document.scene.as_ref().map(|scene| &scene.document), now)?;
        document.tools.advance(now)?;
        if let Some((_, frame)) =
            self.initial_frame.take().filter(|(offered, _)| *offered == intent)
        {
            let outcome = self.document.tools.navigate(frame, now)?;
            if !matches!(outcome, sugarsubstitute_canvas::NavigationCommandOutcome::Applied { .. })
            {
                tracing::debug!(operation="canvas_selection_frame", outcome=?outcome, "Selection camera was not admitted; the retained camera remains");
            }
        }
        if self.pacing.period() != self.document.tools.navigation().frame_period() {
            self.pacing.set_frame_period(self.document.tools.navigation().frame_period(), now)?;
        }
        self.expose_pending = true;
        self.title = self.labels.viewing(self.document.tools.navigation().viewport().zoom());
        Ok(false)
    }

    /// Hand evicted and forgotten documents to the existing retirement worker.
    ///
    /// # Errors
    /// Preserves retirement deadline arithmetic failures; the documents stay retained.
    pub(super) fn advance_retained_retirement(&mut self) -> Result<(), Box<dyn Error>> {
        if self.deferred_retirement.is_some() {
            return Ok(());
        }
        if let Some(retiring) = self.retained.take_retiring() {
            self.retire(Retired { _retained: Some(retiring), ..Default::default() })?;
        }
        Ok(())
    }
}
