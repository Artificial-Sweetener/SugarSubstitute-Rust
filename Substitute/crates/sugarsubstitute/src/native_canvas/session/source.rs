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
        let mut originals = source.take().map(Originals::single);
        let result = self.offer_originals(&mut originals, reuse, expected);
        if let Some(originals) = originals {
            *source = Some(originals.primary);
        }
        result
    }

    /// Offer one original and an optional comparison original as a single selection.
    /// A rejected caller retains both originals for retry or worker-owned disposal.
    ///
    /// # Errors
    /// Preserves the caller's originals on exhausted identity, deadline or admission failure.
    pub(crate) fn offer_originals(
        &mut self,
        originals: &mut Option<Originals>,
        reuse: CacheReuse,
        expected: Option<AssetId>,
    ) -> Result<bool, Box<dyn Error>> {
        let mut input = originals.take().map(OpenSource::Image);
        let result = self.offer_original(&mut input, reuse, expected);
        if let Some(OpenSource::Image(returned)) = input {
            *originals = Some(returned);
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

    /// Map a source intent the host offered to the current selection that continues it.
    ///
    /// A cache-corruption rebuild re-imports the same original under a new selection to fence
    /// the corrupt work, so the rebuilt selection answers the host's request for the original
    /// intent. Any later selection change ends the continuation.
    #[cfg(feature = "native-composition")]
    pub(crate) fn rebuilt_intent(&self, offered: u64) -> Option<u64> {
        self.rebuilt_selection
            .filter(|&(replaced, rebuilt)| replaced == offered && rebuilt == self.selection)
            .map(|(_, rebuilt)| rebuilt)
    }

    /// Report whether the latest offered source is still being acquired or decoded, or a
    /// reselected retained document still waits for earlier document work to settle.
    #[cfg(feature = "native-composition")]
    pub(crate) fn source_loading(&self) -> bool {
        self.pending_open.is_some() || self.loading.is_some() || self.pending_restore.is_some()
    }

    /// Fence the host's current acquisition before its backend or attachment is replaced.
    /// A pending adapter stays retained for worker disposal, and a running job remains
    /// observed until it stops and any native transport has actually retired.
    #[cfg(feature = "native-composition")]
    pub(crate) fn revoke_acquirer(&mut self) -> Result<(), Box<dyn Error>> {
        let revoked = self.cancel_acquirers();
        if revoked {
            if let Some(loading) = &self.loading
                && loading.acquirer.is_some()
            {
                loading.job.cancel();
            }
            self.source_sequence =
                self.source_sequence.checked_add(1).ok_or("Canvas source revision exhausted")?;
            self.selection = self.source_sequence;
            self.abandon_cpu_frame(self.navigation_time())?;
            self.follow_selection();
        }
        Ok(())
    }

    /// Fence a cancelled selection even after acquisition finished and pixels await native acceptance.
    /// Retained acknowledged presentation stays owned; workers and originals retire on their existing
    /// observed paths. A new source intent or document preparation may proceed after that settlement.
    /// The pending inspection is retired with this explicitly cancelled intent.
    ///
    /// # Errors
    /// Rejects exhausted source identities or inconsistent pending CPU/pacing custody.
    #[cfg(feature = "native-composition")]
    pub(crate) fn cancel_selection(&mut self) -> Result<(), Box<dyn Error>> {
        let next = self.source_sequence.checked_add(1).ok_or("Canvas source revision exhausted")?;
        self.abandon_cpu_frame(self.navigation_time())?;
        #[cfg(windows)]
        self.abandon_composition_frame(self.navigation_time())?;
        self.cancel_acquirers();
        if let Some(loading) = &self.loading {
            loading.job.cancel();
        }
        if let Some(rendering) = &mut self.rendering {
            rendering.superseded = true;
            rendering.job.cancel();
        }
        if let Some(displaying) = &self.displaying {
            displaying.job.cancel();
        }
        if let Some(presenting) = &self.presenting {
            presenting.job.cancel();
        }
        self.source_sequence = next;
        self.selection = next;
        self.pending_first_image = None;
        self.pending_restore = None;
        self.initial_frame = None;
        self.follow_selection();
        Ok(())
    }

    /// Stop external acquisition owners without dropping their worker/native custody.
    #[cfg(feature = "native-composition")]
    fn cancel_acquirers(&self) -> bool {
        let mut revoked = false;
        if let Some(Open { source: OpenSource::Acquired(owner), .. }) = &self.pending_open {
            owner.cancel();
            revoked = true;
        }
        if let Some(loading) = &self.loading
            && let Some(owner) = &loading.acquirer
        {
            owner.cancel();
            revoked = true;
        }
        revoked
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
        let Some(intent) = self.supersede_pending_source()? else {
            return Ok(false);
        };
        let source = source.take().ok_or("Canvas source is unavailable")?;
        self.pending_open = Some(Open {
            source,
            intent,
            reuse,
            expected,
            #[cfg(feature = "native-composition")]
            retention: None,
        });
        self.title = self.labels.opening.to_owned();
        Ok(true)
    }

    /// Start the next source selection and fence every source-dependent job of the previous one.
    /// An unadmitted original retires on the existing storage worker; running loads, frames and
    /// presentations are cancelled and remain observed until their terminal outcomes.
    /// Returns the new selection, or `None` without any change while the retirement slot that
    /// must receive a superseded original is occupied.
    ///
    /// # Errors
    /// Rejects exhausted source identities, deadline arithmetic and inconsistent CPU custody.
    pub(super) fn supersede_pending_source(&mut self) -> Result<Option<u64>, Box<dyn Error>> {
        let next = self.source_sequence.checked_add(1).ok_or("Canvas source revision exhausted")?;
        let retiring = self
            .pending_open
            .as_ref()
            .is_some_and(|pending| pending.source.requires_worker_retirement());
        if retiring && self.pending_retirement.is_some() {
            return Ok(None);
        }
        self.abandon_cpu_frame(self.navigation_time())?;
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
        let previous = self.pending_open.take();
        if retiring {
            self.pending_retirement = Some(Retired { _open: previous, ..Default::default() });
            self.retirement_deadline = self.disposal_cutoff();
        }
        #[cfg(feature = "native-composition")]
        {
            self.pending_restore = None;
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
        #[cfg(feature = "native-composition")]
        self.follow_selection();
        self.failed = None;
        Ok(Some(next))
    }
}
