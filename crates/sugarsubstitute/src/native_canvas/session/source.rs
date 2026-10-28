// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit the latest canvas original while preserving superseded native custody.

use super::*;

impl Session {
    /// Record a source selection before cancelling old work; the old document remains retained.
    pub(super) fn open(
        &mut self,
        path: PathBuf,
        reuse: CacheReuse,
        expected: Option<AssetId>,
    ) -> Result<(), Box<dyn Error>> {
        let mut source = Some(ImageSource::file(path));
        if !self.offer_source(&mut source, reuse, expected)? {
            return Err("Canvas source retirement is still pending".into());
        }
        Ok(())
    }
    /// Offer one original without taking it when closing or disposal capacity is unavailable.
    /// A rejected caller retains its input for retry or worker-owned disposal. A cached
    /// original's final handle is released only by the existing storage worker.
    ///
    /// # Errors
    /// Preserves the caller's source on exhausted identity, deadline or admission failure.
    pub(crate) fn offer_source(
        &mut self,
        source: &mut Option<ImageSource>,
        reuse: CacheReuse,
        expected: Option<AssetId>,
    ) -> Result<bool, Box<dyn Error>> {
        let mut input = source.take().map(OpenSource::Image);
        let result = self.offer_original(&mut input, reuse, expected);
        if let Some(OpenSource::Image(original)) = input {
            *source = Some(original);
        }
        result
    }

    /// Offer an inert host adapter without taking it when source admission is unavailable.
    #[cfg(feature = "native-composition")]
    pub(crate) fn offer_acquirer(
        &mut self,
        source: &mut Option<Arc<dyn acquisition::SourceAcquirer>>,
        reuse: CacheReuse,
    ) -> Result<bool, Box<dyn Error>> {
        let mut input = source.take().map(OpenSource::Acquired);
        let result = self.offer_original(&mut input, reuse, None);
        if let Some(OpenSource::Acquired(original)) = input {
            *source = Some(original);
        }
        result
    }

    /// Fence the host's current acquisition before its backend or attachment is replaced.
    /// A pending adapter stays retained for worker disposal, and a running job remains
    /// observed until it stops and any native transport has actually retired.
    #[cfg(feature = "native-composition")]
    pub(crate) fn revoke_acquirer(&mut self) -> Result<(), Box<dyn Error>> {
        let mut revoked = false;
        if let Some(Open { source: OpenSource::Acquired(owner), .. }) = &self.pending_open {
            owner.cancel();
            revoked = true;
        }
        if let Some(loading) = &self.loading
            && let Some(owner) = &loading.acquirer
        {
            owner.cancel();
            loading.job.cancel();
            revoked = true;
        }
        if revoked {
            self.source_sequence =
                self.source_sequence.checked_add(1).ok_or("Canvas source revision exhausted")?;
            self.selection = self.source_sequence;
            self.abandon_cpu_frame(self.navigation_time())?;
        }
        Ok(())
    }

    /// Assign one canvas source generation while preserving the caller's unaccepted input.
    fn offer_original(
        &mut self,
        source: &mut Option<OpenSource>,
        reuse: CacheReuse,
        expected: Option<AssetId>,
    ) -> Result<bool, Box<dyn Error>> {
        if self.closing {
            return Ok(false);
        }
        if source.is_none() {
            return Err("Canvas source is unavailable".into());
        }
        let next = self.source_sequence.checked_add(1).ok_or("Canvas source revision exhausted")?;
        let retiring = self
            .pending_open
            .as_ref()
            .is_some_and(|pending| pending.source.requires_worker_retirement());
        let cutoff = if retiring {
            if self.pending_retirement.is_some() {
                return Ok(false);
            }
            Some(self.retirement_cutoff()?)
        } else {
            None
        };
        self.abandon_cpu_frame(self.navigation_time())?;
        let source = source.take().ok_or("Canvas source is unavailable")?;
        self.input_observed = self.navigation_time();
        self.selection = next;
        self.source_sequence = next;
        if self.first_image_queue_latency.is_none() {
            self.pending_first_image = Some((self.selection, self.input_observed));
        }
        #[cfg(feature = "native-composition")]
        if let Some(Open { source: OpenSource::Acquired(owner), .. }) = &self.pending_open {
            owner.cancel();
        }
        let previous =
            self.pending_open.replace(Open { source, intent: self.selection, reuse, expected });
        if let Some(cutoff) = cutoff {
            self.pending_retirement = Some(Retired { _open: previous, ..Default::default() });
            self.retirement_deadline = Some(cutoff);
        }
        if let Some(loading) = &self.loading {
            #[cfg(feature = "native-composition")]
            if let Some(owner) = &loading.acquirer {
                owner.cancel();
            }
            loading.job.cancel();
        }
        if let Some(rendering) = &self.rendering {
            rendering.job.cancel();
        }
        if let Some(presenting) = &self.presenting {
            presenting.job.cancel();
        }
        self.failed = None;
        self.title = self.labels.opening.to_owned();
        Ok(true)
    }
}
