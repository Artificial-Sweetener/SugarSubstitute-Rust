// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The close save's final input-document save and its verdict.
//!
//! The application's close save begins it while the window is still up, before the exit
//! intent; canvas close begins it too when no close save did. It captures every unsaved change
//! once, saves it as one owned job bounded by the graceful cutoff, and settles into the
//! [`SaveVerdict`] the close save collects.
use super::*;
/// The close save's input documents: their cutoff and how far their final save has come.
pub(super) struct FinalSave {
    pub(super) cutoff: Moment,
    pub(super) began: Moment,
    pub(super) stage: FinalStage,
}

/// Unsaved content captured at close waiting for a running save, the running save that holds
/// the latest content, or the settled verdict.
pub(super) enum FinalStage {
    Waiting(Box<InputDocumentArchive>),
    Running,
    Settled(SaveVerdict),
}

impl Session {
    /// At canvas close, begin the final save under the shutdown cutoff unless the application's
    /// close save already began it.
    pub(in crate::native_canvas::session) fn close_input_documents(&mut self) {
        if let Some(cutoff) = self.shutdown_deadline {
            self.begin_final_input_save(cutoff);
        }
    }

    /// Begin the close save's final save while the documents are still in place: capture every
    /// unsaved change and save it as an owned job bounded by `cutoff`, on the execution clock.
    /// A save already running finishes first; observing it submits the captured final save.
    /// Automatic saving stops here. A repeated call only tightens the cutoff.
    pub(crate) fn begin_final_input_save(&mut self, cutoff: Moment) {
        let now = self.clock.now();
        let signature = self.input_signature();
        let Some(persistence) = self.inputs.persistence.as_mut() else { return };
        if let Some(final_save) = &mut persistence.final_save {
            final_save.cutoff = final_save.cutoff.min(cutoff);
            self.submit_final_save();
            return;
        }
        let Some(autosave) = persistence.autosave.as_mut() else {
            // The startup load has not finished, so nothing was authored in this run.
            persistence.final_save = Some(FinalSave {
                cutoff,
                began: now,
                stage: FinalStage::Settled(SaveVerdict::Saved),
            });
            return;
        };
        // A change since the last poll has not reached the schedule yet.
        if persistence.signature.replace(signature).is_some_and(|previous| previous != signature) {
            persistence.generation += 1;
            persistence.unsaved_since.get_or_insert(now);
            if let Err(cause) = autosave.changed(persistence.generation, now) {
                tracing::warn!(operation = "input_documents_close_save", cause_chain = %crate::diagnostics::CauseChain(&cause), "A final input-document change could not be scheduled");
            }
        }
        let stage = match (autosave.dirty(), autosave.saving()) {
            (false, false) => FinalStage::Settled(SaveVerdict::Saved),
            // The running save holds the latest content; its outcome is the verdict.
            (false, true) => FinalStage::Running,
            (true, _) => match self.input_archive() {
                Ok(archive) => FinalStage::Waiting(Box::new(archive)),
                Err(cause) => {
                    tracing::warn!(operation = "input_documents_close_save", cause_chain = %crate::diagnostics::CauseChain(&cause), "The final input documents could not be captured; the previous archive is kept");
                    FinalStage::Settled(SaveVerdict::Failed(SaveFailure::Capture))
                }
            },
        };
        if let Some(persistence) = self.inputs.persistence.as_mut() {
            persistence.final_save = Some(FinalSave { cutoff, began: now, stage });
        }
        self.submit_final_save();
    }

    /// What the final save kept, once it settled or its cutoff passed at `now`; `None` while
    /// it runs or before it began. Without input persistence nothing can be lost.
    pub(crate) fn input_close_verdict(&self, now: Moment) -> Option<SaveVerdict> {
        let Some(persistence) = &self.inputs.persistence else { return Some(SaveVerdict::Saved) };
        let final_save = persistence.final_save.as_ref()?;
        match final_save.stage {
            FinalStage::Settled(verdict) => Some(verdict),
            _ if now >= final_save.cutoff => Some(SaveVerdict::Stale {
                since: persistence.unsaved_since.unwrap_or(final_save.began).min(now),
            }),
            _ => None,
        }
    }

    /// Submit the captured final save once no other save is running.
    pub(super) fn submit_final_save(&mut self) {
        let now = self.clock.now();
        let Some(persistence) = self.inputs.persistence.as_mut() else { return };
        let Some(final_save) = persistence.final_save.as_mut() else { return };
        if !matches!(final_save.stage, FinalStage::Waiting(_)) {
            return;
        }
        let cutoff = final_save.cutoff;
        // A save admitted at or past its cutoff could not finish: the captured content is
        // released and the verdict is stale.
        if now >= cutoff {
            let since = persistence.unsaved_since.unwrap_or(final_save.began).min(now);
            final_save.stage = FinalStage::Settled(SaveVerdict::Stale { since });
            return;
        }
        if persistence.autosave.as_ref().is_none_or(Autosave::saving) {
            return;
        }
        let FinalStage::Waiting(archive) =
            std::mem::replace(&mut final_save.stage, FinalStage::Running)
        else {
            return;
        };
        let Some(autosave) = persistence.autosave.as_mut() else { return };
        let flushed = match self.runtime.as_ref() {
            Some(runtime) => {
                autosave.flush(now, cutoff, &runtime.client, &runtime.scope, &self.sequence, || {
                    Ok(*archive)
                })
            }
            None => Ok(None),
        };
        let running = autosave.saving();
        let dirty = autosave.dirty();
        let settled = match flushed {
            Ok(_) if running => None,
            Ok(Some(SaveOutcome::Unchanged)) | Ok(None) if !dirty => Some(SaveVerdict::Saved),
            Ok(_) => Some(SaveVerdict::Failed(SaveFailure::Storage)),
            Err(cause) => {
                tracing::warn!(operation = "input_documents_close_save", cause_chain = %crate::diagnostics::CauseChain(&cause), "The final input-document save could not start; the previous archive is kept");
                Some(SaveVerdict::Failed(match cause {
                    crate::input_documents::AutosaveError::Capture(_) => SaveFailure::Capture,
                    _ => SaveFailure::Storage,
                }))
            }
        };
        if let Some(verdict) = settled {
            if verdict.saved() {
                persistence.unsaved_since = None;
            }
            final_save.stage = FinalStage::Settled(verdict);
        }
    }
}
