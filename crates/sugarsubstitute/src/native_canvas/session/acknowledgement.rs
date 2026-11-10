// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify native presentation acknowledgements that the session cannot take as proof of display.
//!
//! The composition acknowledges the exact frame it showed. Most mismatches are ordinary races with
//! newer work and change nothing; a few mean that the shown frame cannot become accepted evidence.
//! Neither ends the application. The session answers with a typed [`Unaccepted`] before it mutates
//! any custody, so its owner can drop the frame and ask for another. Errors that carry no
//! classification remain custody or accounting corruption.

use super::*;
use std::fmt;

/// Why an acknowledgement did not complete a presentation, with custody still unchanged.
#[derive(Debug)]
pub(crate) enum Unaccepted {
    /// The acknowledgement names no frame this session awaits.
    Ignored(Ignored),
    /// The acknowledgement names the awaited frame, which cannot become accepted evidence.
    Rejected(Rejected),
}

impl Unaccepted {
    /// Reject the awaited frame for `reason`.
    pub(crate) const fn rejected(reason: Rejection) -> Self {
        Self::Rejected(Rejected { reason, cause: None })
    }

    /// Reject the awaited frame for `reason`, keeping the failure that proved it unacceptable.
    pub(crate) fn rejected_by(reason: Rejection, cause: Box<dyn Error>) -> Self {
        Self::Rejected(Rejected { reason, cause: Some(cause) })
    }
}

impl fmt::Display for Unaccepted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ignored(ignored) => write!(f, "Native canvas acknowledgement ignored: {ignored}"),
            Self::Rejected(rejected) => rejected.fmt(f),
        }
    }
}

impl Error for Unaccepted {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Ignored(_) => None,
            Self::Rejected(rejected) => rejected.source(),
        }
    }
}

/// An acknowledgement that matches no awaited frame; it leaves every state as it was.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Ignored {
    /// A frame the session has since replaced, or a presentation it no longer shows.
    #[cfg(feature = "native-composition")]
    Stale,
    /// A frame this session never offered for acknowledgement, or offered and already settled.
    Foreign,
    /// A frame acknowledged while the surface awaits the native hide of its peer.
    #[cfg(feature = "native-composition")]
    HidePending,
}

impl fmt::Display for Ignored {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            #[cfg(feature = "native-composition")]
            Self::Stale => "stale frame",
            Self::Foreign => "foreign frame",
            #[cfg(feature = "native-composition")]
            Self::HidePending => "hide pending",
        })
    }
}

/// The awaited frame, shown by the composition, that cannot become accepted evidence.
#[derive(Debug)]
pub(crate) struct Rejected {
    reason: Rejection,
    cause: Option<Box<dyn Error>>,
}

impl Rejected {
    /// The class of evidence that failed.
    #[cfg(feature = "native-composition")]
    pub(crate) const fn reason(&self) -> Rejection {
        self.reason
    }
}

impl fmt::Display for Rejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason.message())
    }
}

impl Error for Rejected {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause.as_deref()
    }
}

/// Which evidence or ownership fact an acknowledged frame failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Rejection {
    /// Display evidence does not prove complete current coverage of the shown document.
    Evidence,
    /// The accepted camera does not match the navigation content it must freeze.
    #[cfg(feature = "native-composition")]
    Camera,
    /// The CPU or shared frame's evidence or projection custody is absent.
    MissingEvidence,
    /// The frame was drawn for a backdrop generation that has since changed.
    StaleBackdrop,
    /// The selected frame is no longer available to present.
    #[cfg(feature = "native-composition")]
    FrameUnavailable,
    /// The attachment's binding, placement or allocation is gone.
    #[cfg(feature = "native-composition")]
    BindingGone,
}

impl Rejection {
    /// Closed diagnostic name, free of paths and content.
    #[cfg(feature = "native-composition")]
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Evidence => "evidence",
            Self::Camera => "camera",
            Self::MissingEvidence => "missing_evidence",
            Self::StaleBackdrop => "stale_backdrop",
            Self::FrameUnavailable => "frame_unavailable",
            Self::BindingGone => "binding_gone",
        }
    }

    const fn message(self) -> &'static str {
        match self {
            Self::Evidence => "Canvas display evidence does not prove complete current coverage",
            #[cfg(feature = "native-composition")]
            Self::Camera => "Accepted camera does not match navigation content",
            Self::MissingEvidence => "Canvas presentation evidence is unavailable",
            Self::StaleBackdrop => "Canvas display evidence has a stale backdrop generation",
            #[cfg(feature = "native-composition")]
            Self::FrameUnavailable => "Selected canvas frame is unavailable",
            #[cfg(feature = "native-composition")]
            Self::BindingGone => "Canvas binding, placement or allocation disappeared",
        }
    }
}

impl Session {
    /// Judge display evidence against the current document without accepting it.
    pub(super) fn judge_display(&self, evidence: DisplayEvidence) -> Result<bool, Unaccepted> {
        self.judge_display_in(&self.document, evidence)
    }

    /// Judge display evidence against `document` without accepting it.
    ///
    /// Returns whether the evidence proves the document's current content; evidence of an
    /// earlier revision or gesture step of the same document proves nothing and is not an error.
    pub(super) fn judge_display_in(
        &self,
        document: &document::DocumentState,
        evidence: DisplayEvidence,
    ) -> Result<bool, Unaccepted> {
        if evidence.backdrop_generation != self.backdrops.generation {
            return Err(Unaccepted::rejected(Rejection::StaleBackdrop));
        }
        let scene =
            document.scene.as_ref().ok_or_else(|| Unaccepted::rejected(Rejection::Evidence))?;
        let (key, snapshot) =
            document.shown().ok_or_else(|| Unaccepted::rejected(Rejection::Evidence))?;
        if evidence.document != key
            && evidence.document.0 == key.0
            && evidence.source_revision == scene.source_revision
        {
            return Ok(false);
        }
        if evidence.document != key || evidence.source_revision != scene.source_revision {
            return Err(Unaccepted::rejected(Rejection::Evidence));
        }
        let complete = evidence
            .proves_complete(snapshot.extent())
            .map_err(|cause| Unaccepted::rejected_by(Rejection::Evidence, cause))?;
        if complete { Ok(true) } else { Err(Unaccepted::rejected(Rejection::Evidence)) }
    }

    /// Release the custody of a frame whose acknowledgement was rejected and demand a fresh one.
    ///
    /// The previously accepted evidence and pixels stay the session's accepted display. The
    /// demand reuses the ordinary redraw path, so the next frame follows normal readiness and
    /// pacing. A target the session no longer holds changes nothing but the demand.
    ///
    /// # Errors
    /// Preserves custody when pending frame evidence or its pacing identity is inconsistent.
    #[cfg(feature = "native-composition")]
    pub(crate) fn release_rejected_frame(
        &mut self,
        target: FrameStamp,
    ) -> Result<(), Box<dyn Error>> {
        if self.preview_frame_offered(target) {
            self.withdraw_preview()?;
        }
        if self.document.cpu_pending == Some(target) {
            // The evidence itself may be what went missing, so custody is released by identity.
            self.pacing.abandoned(target, self.navigation_time())?;
            self.document.cpu_pending = None;
            self.document.pending_cpu_evidence = None;
        }
        #[cfg(all(windows, feature = "native-composition"))]
        if self.composition_pending.as_ref().is_some_and(|display| display.target == target) {
            self.abandon_composition_frame(self.navigation_time())?;
        }
        self.expose_pending = true;
        Ok(())
    }
}

#[cfg(all(test, feature = "native-composition"))]
mod faults;
