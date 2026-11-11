// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Persist the input documents in the session directory and restore them at startup.
//!
//! At startup the archive loads before any workflow sync is admitted. Its host metadata
//! restores every workflow's surfaces with their document and layer identities, synced mask
//! state and the active input document. Blank-based documents are rebuilt at once; a file-based
//! document keeps its archived layers until the next sync re-acquires its base image, and then
//! keeps its masks when the image's size is unchanged. An archive the store rejects, or whose
//! metadata is invalid, leaves no documents: the next sync rebuilds them from the workflow.
//!
//! After that, every change to the documents or their synced state advances a content
//! generation that [`Autosave`] coalesces into one save of only changed content. A brush stroke
//! commits to its document when it ends, so the stroke's end is the change that schedules its
//! save: a quarter second later, and never more than two seconds after the oldest unsaved
//! change while strokes keep arriving. A hard kill therefore loses at most that window.
//!
//! The application's close save begins the final save while the window is still up, before
//! the exit intent: it captures whatever is unsaved and saves it as an owned job bounded by the
//! graceful cutoff, and answers with a [`SaveVerdict`]. A save that cannot finish by then
//! leaves the previous archive intact and is reported stale; a failed write is reported failed.
mod final_save;
mod metadata;
mod workspace;

use super::*;
use crate::input_documents::{
    Autosave, AutosavePolicy, InputDocumentStore, Restored, SaveOutcome, StoreError,
};
use final_save::{FinalSave, FinalStage};
use std::hash::{DefaultHasher, Hash, Hasher};
use sugarsubstitute_application::workspace_session::close_save::{SaveFailure, SaveVerdict};
use sugarsubstitute_canvas::{
    ArchiveError, ArchivedDocument, InputDocumentArchive, LayerContent, SourceReference,
};

/// Debounce of automatic saves, matching the workspace checkpoint's longest delay.
pub(crate) const POLICY: AutosavePolicy =
    AutosavePolicy { quiet: Duration::from_millis(250), longest: Duration::from_secs(2) };

/// Startup restore and automatic saving of one session directory.
pub(in crate::native_canvas::session) struct Persistence {
    store: InputDocumentStore,
    source: Option<InputDocumentStore>,
    load: Load,
    autosave: Option<Autosave>,
    generation: u64,
    signature: Option<u64>,
    /// When the oldest change not yet durable was made.
    unsaved_since: Option<Moment>,
    /// The final save of the close save, once it began.
    final_save: Option<FinalSave>,
}

enum Load {
    WaitingHost,
    Pending,
    Running(OwnedJob<Restored, StoreError>),
    Done,
}

impl Persistence {
    /// Whether the startup load or a save awaits its terminal outcome.
    pub(super) fn in_flight(&self) -> bool {
        matches!(self.load, Load::Running(_))
            || self.autosave.as_ref().is_some_and(Autosave::saving)
    }

    /// Cancel the startup load; its outcome is still observed. A running save continues so the
    /// final save can follow it.
    pub(super) fn close(&mut self) {
        if let Load::Running(job) = &self.load {
            job.cancel();
        }
    }
}

impl Session {
    /// Persist input documents in `store`'s session directory. Call before the canvas runtime
    /// starts; the archive loads once canvas storage is ready.
    pub(crate) fn persist_inputs(
        &mut self,
        store: InputDocumentStore,
        plan: crate::input_documents::InputRestorePlan,
    ) {
        let waiting = matches!(plan, crate::input_documents::InputRestorePlan::AwaitingAssociation);
        let source = match plan {
            crate::input_documents::InputRestorePlan::Verified(source) => Some(source),
            _ => None,
        };
        self.inputs.persistence = Some(Persistence {
            store,
            source,
            load: if waiting { Load::WaitingHost } else { Load::Pending },
            autosave: None,
            generation: 0,
            signature: None,
            unsaved_since: None,
            final_save: None,
        });
    }
    /// Resolve the existing host-association hold before admitting any workflow input sync.
    /// # Errors
    /// Refuses duplicate resolution or a second unresolved plan.
    pub(crate) fn resolve_input_association(
        &mut self,
        plan: crate::input_documents::InputRestorePlan,
    ) -> Result<(), Box<dyn Error>> {
        let persistence =
            self.inputs.persistence.as_mut().ok_or("Input persistence unavailable")?;
        if !matches!(persistence.load, Load::WaitingHost) {
            return Err("Input association is already resolved".into());
        }
        persistence.source = match plan {
            crate::input_documents::InputRestorePlan::Current => None,
            crate::input_documents::InputRestorePlan::Verified(source) => Some(source),
            crate::input_documents::InputRestorePlan::AwaitingAssociation => {
                return Err("Input association remains unresolved".into());
            }
        };
        persistence.load = Load::Pending;
        Ok(())
    }

    /// Whether archived documents, if any, are restored, so workflow syncs may proceed.
    pub(crate) fn inputs_restored(&self) -> bool {
        self.inputs
            .persistence
            .as_ref()
            .is_none_or(|persistence| matches!(persistence.load, Load::Done))
    }

    /// Start the startup load once canvas storage exists.
    pub(in crate::native_canvas::session) fn submit_input_restore(
        &mut self,
    ) -> Result<(), Box<dyn Error>> {
        if self.closing
            || !self
                .inputs
                .persistence
                .as_ref()
                .is_some_and(|persistence| matches!(persistence.load, Load::Pending))
        {
            return Ok(());
        }
        let memory = self.resources.as_ref().ok_or("Canvas storage is unavailable")?.memory.clone();
        let spec = self.event_spec(
            Operation::Infrastructure,
            JobClass::InfrastructureOrdinary,
            InputDocumentStore::load_reservation(),
            None,
        )?;
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let persistence = self.inputs.persistence.as_mut().ok_or("Input persistence vanished")?;
        match persistence.source.as_ref().unwrap_or(&persistence.store).submit_load(
            &runtime.client,
            &runtime.scope,
            spec,
            memory,
        ) {
            Ok(job) => persistence.load = Load::Running(job),
            Err(cause) if activity::retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }

    /// Observe the startup load and a finished save.
    pub(in crate::native_canvas::session) fn consume_input_persistence(
        &mut self,
    ) -> Result<(), Box<dyn Error>> {
        let now = self.clock.now();
        let Some(persistence) = self.inputs.persistence.as_mut() else { return Ok(()) };
        if let Some(autosave) = &mut persistence.autosave
            && let Some(outcome) = autosave.observe(now)?
        {
            let dirty = autosave.dirty();
            let closing = persistence.final_save.is_some();
            match &outcome {
                SaveOutcome::Saved(receipt) => tracing::debug!(
                    operation = "input_documents_autosave",
                    bytes = receipt.bytes,
                    "Input documents saved"
                ),
                SaveOutcome::Unchanged | SaveOutcome::Failed(_) => {}
                SaveOutcome::Stopped(kind) if closing => {
                    tracing::warn!(operation = "input_documents_close_save", outcome = ?kind, "The final input-document save did not finish within shutdown; the previous archive is kept")
                }
                SaveOutcome::Stopped(kind) => {
                    tracing::debug!(operation = "input_documents_autosave", outcome = ?kind, "Input document save stopped")
                }
            }
            let durable = matches!(outcome, SaveOutcome::Saved(_) | SaveOutcome::Unchanged);
            if durable && !dirty {
                persistence.unsaved_since = None;
            }
            if let Some(final_save) = &mut persistence.final_save {
                if matches!(final_save.stage, FinalStage::Running) {
                    let since = persistence.unsaved_since.unwrap_or(final_save.began);
                    final_save.stage = FinalStage::Settled(match outcome {
                        _ if durable && !dirty => SaveVerdict::Saved,
                        SaveOutcome::Failed(_) => SaveVerdict::Failed(SaveFailure::Storage),
                        _ => SaveVerdict::Stale { since },
                    });
                }
                self.submit_final_save();
                return Ok(());
            }
        }
        let Load::Running(job) = &mut persistence.load else { return Ok(()) };
        let Some(completion) = job.try_take_completion() else { return Ok(()) };
        persistence.load = Load::Done;
        let archive = match completion.outcome {
            JobOutcome::Success(restored) => {
                for rejection in &restored.rejected {
                    tracing::warn!(operation = "input_documents_restore", file = ?rejection.file, cause_chain = %crate::diagnostics::CauseChain(&rejection.cause), "An input-document archive was rejected; its file is kept");
                }
                restored.archive.map(|(_, archive)| archive)
            }
            JobOutcome::Failure(cause) => {
                tracing::warn!(operation = "input_documents_restore", cause_chain = %crate::diagnostics::CauseChain(&cause), "Input documents could not be loaded; they are rebuilt from the workflow");
                None
            }
            other => {
                tracing::warn!(operation = "input_documents_restore", outcome = ?other.kind(), "Input document loading stopped; they are rebuilt from the workflow");
                None
            }
        };
        let restored = match archive {
            Some(archive) if !self.closing => match self.restore_inputs(&archive) {
                Ok(()) => Some(archive),
                Err(cause) => {
                    tracing::warn!(operation = "input_documents_restore", cause_chain = %crate::diagnostics::CauseChain(cause.as_ref()), "Archived input documents are inconsistent; they are rebuilt from the workflow");
                    self.inputs.workflows.clear();
                    None
                }
            },
            _ => None,
        };
        let persistence = self.inputs.persistence.as_mut().ok_or("Input persistence vanished")?;
        persistence.autosave =
            Some(Autosave::new(persistence.store.clone(), POLICY, 0, restored.as_ref()));
        persistence.signature = None;
        Ok(())
    }

    /// Advance the content generation after any change and start a due save.
    pub(in crate::native_canvas::session) fn advance_input_autosave(
        &mut self,
    ) -> Result<(), Box<dyn Error>> {
        let saving = self.inputs.persistence.as_ref().is_some_and(|persistence| {
            persistence.autosave.is_some() && persistence.final_save.is_none()
        });
        if self.closing || !saving {
            return Ok(());
        }
        let signature = self.input_signature();
        let now = self.clock.now();
        let Some(persistence) = self.inputs.persistence.as_mut() else { return Ok(()) };
        let Some(autosave) = persistence.autosave.as_mut() else { return Ok(()) };
        match persistence.signature.replace(signature) {
            None => {}
            Some(previous) if previous == signature => {}
            Some(_) => {
                persistence.generation += 1;
                persistence.unsaved_since.get_or_insert(now);
                autosave.changed(persistence.generation, now)?;
            }
        }
        if autosave.deadline().is_none_or(|deadline| deadline > now) {
            return Ok(());
        }
        let mut autosave = persistence.autosave.take().ok_or("Autosave vanished")?;
        let polled = match self.runtime.as_ref() {
            Some(runtime) => {
                autosave.poll(now, &runtime.client, &runtime.scope, &self.sequence, || {
                    self.input_archive()
                })
            }
            None => Ok(None),
        };
        if let Some(persistence) = self.inputs.persistence.as_mut() {
            if matches!(polled, Ok(Some(SaveOutcome::Unchanged))) && !autosave.dirty() {
                persistence.unsaved_since = None;
            }
            persistence.autosave = Some(autosave);
        }
        if let Err(cause) = polled {
            tracing::warn!(operation = "input_documents_autosave", cause_chain = %crate::diagnostics::CauseChain(&cause), "Input documents remain unsaved");
        }
        Ok(())
    }

    /// When the next automatic save is due.
    pub(in crate::native_canvas::session) fn input_autosave_deadline(&self) -> Option<Moment> {
        self.inputs.persistence.as_ref()?.autosave.as_ref()?.deadline()
    }

    /// Every input document's current content, with the synced state that maps it.
    fn input_archive(&self) -> Result<InputDocumentArchive, ArchiveError> {
        let mut documents = Vec::new();
        for surface in self.inputs.workflows.iter().flat_map(|(_, surfaces)| surfaces) {
            let snapshot = self
                .input_document(surface.identity)
                .and_then(|document| document.scene.as_ref())
                .filter(|_| surface.loaded)
                .map(|scene| scene.document.snapshot());
            match (snapshot, &surface.restored) {
                (Some(snapshot), _) => {
                    let reference = match &surface.loaded_base {
                        Some(Base::File(reference)) => {
                            Some(SourceReference::new(reference.as_bytes())?)
                        }
                        _ => None,
                    };
                    let key = surface.identity.protocol_id();
                    documents
                        .push(ArchivedDocument::capture(&key, &snapshot, |_| reference.clone())?);
                }
                (None, Some(restored)) => documents.push(restored.clone()),
                (None, None) => {}
            }
        }
        let active = self.document.input.map(|document| (document, self.document.tools.target()));
        InputDocumentArchive::new(documents, &metadata::encode(&self.inputs, active)?)
    }

    /// Rebuild the archived surfaces; documents of blank-based surfaces are restored at once.
    fn restore_inputs(&mut self, archive: &InputDocumentArchive) -> Result<(), Box<dyn Error>> {
        let restored = metadata::decode(archive.host_metadata())?;
        let memory = self.resources.as_ref().ok_or("Canvas storage is unavailable")?.memory.clone();
        self.inputs.next_identity = self.inputs.next_identity.max(restored.next_identity);
        for (workflow, mut surfaces) in restored.workflows {
            for surface in &mut surfaces {
                let Some(document) = archive.document(&surface.identity.protocol_id()) else {
                    continue;
                };
                // A blank backing's size changes only through resizes, so the archived extent
                // is the surface's current one.
                if matches!(surface.desired_base, Base::Blank(_)) {
                    let mut state = self.restored_document(surface.identity, document, &memory)?;
                    surface.reconcile(&mut state, &memory)?;
                    surface.document = Some(state);
                    surface.loaded = true;
                    surface.loaded_base = Some(Base::Blank(document.extent()));
                } else {
                    surface.restored = Some(document.clone());
                }
            }
            self.inputs.workflows.push((workflow, surfaces));
        }
        if let Some((document, target)) = restored.active
            && self.inputs.by_identity(document).is_some()
        {
            self.inputs.route = Some(route::Pending::Input { surface: document, target });
        }
        Ok(())
    }

    /// A parked document holding archived content under `identity`, with empty history.
    fn restored_document(
        &self,
        identity: InputDocumentId,
        archived: &ArchivedDocument,
        memory: &MemoryLease,
    ) -> Result<Box<document::DocumentState>, Box<dyn Error>> {
        let now = self.navigation_time();
        let document = archived.restore(identity.get(), memory)?;
        let viewport = Viewport::new(self.document.tools.navigation().viewport().surface())?;
        let mut state = Box::new(document::DocumentState::new(viewport));
        state.tools.set_refresh_rate(self.display_refresh)?;
        state.tools.set_content(Some(archived.extent()), Some(&document), now)?;
        state.scene = Some(Scene {
            document,
            source: None,
            source_revision: 0,
            original: None,
            compared: None,
            placed: Vec::new(),
        });
        state.input = Some(identity);
        state.initial_preview = true;
        Ok(state)
    }

    /// A process-local digest of everything an archive would record.
    fn input_signature(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        for (workflow, surfaces) in &self.inputs.workflows {
            workflow.hash(&mut hasher);
            for surface in surfaces {
                metadata::hash_surface(surface, &mut hasher);
                let revision = self
                    .input_document(surface.identity)
                    .and_then(|document| document.scene.as_ref())
                    .map(|scene| scene.document.snapshot().revision());
                revision.hash(&mut hasher);
            }
        }
        self.document.input.hash(&mut hasher);
        self.document.tools.target().map(LayerId::get).hash(&mut hasher);
        hasher.finish()
    }
}

impl Surface {
    /// Replace a freshly created document's content with this surface's archived layers and
    /// selection when the new base has the archived extent, keeping the new base layer. Content
    /// the memory budget cannot hold stays archived for the surface's next document.
    pub(super) fn adopt_restored(
        &mut self,
        scene: &mut Scene,
        memory: &MemoryLease,
    ) -> Result<(), Box<dyn Error>> {
        let Some(archived) = self.restored.take() else { return Ok(()) };
        let snapshot = scene.document.snapshot();
        if archived.extent() != snapshot.extent() {
            return Ok(());
        }
        let base = LayerId::new(BASE_LAYER)?;
        let mut layers = snapshot
            .layers()
            .iter()
            .filter(|layer| layer.id() == base)
            .cloned()
            .collect::<Vec<_>>();
        layers.extend(
            archived
                .layers()
                .iter()
                .filter(|layer| {
                    layer.id() != base && matches!(layer.content(), LayerContent::Coverage { .. })
                })
                .cloned(),
        );
        match sugarsubstitute_canvas::Document::restore(
            snapshot.identity(),
            snapshot.extent(),
            &layers,
            archived.selection().cloned(),
            memory,
        ) {
            Ok(document) => scene.document = document,
            Err(cause) => {
                self.restored = Some(archived);
                return Err(cause.into());
            }
        }
        Ok(())
    }
}
