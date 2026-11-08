// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expose actual native archive snapshots and validate their workspace associations.
use super::*;
impl Session {
    /// Match only real codec documents and the native metadata owner's workflow/surface/mask.
    pub(crate) fn archive_selection(
        archive: &InputDocumentArchive,
        workflow: &str,
        surface: Option<&str>,
        mask: Option<&str>,
    ) -> bool {
        metadata::decode(archive.host_metadata()).is_ok_and(|metadata| {
            metadata
                .workflows
                .iter()
                .filter(|(id, _)| &**id == workflow)
                .flat_map(|(_, surfaces)| surfaces)
                .any(|native| {
                    surface.is_none_or(|key| key == &*native.key)
                        && mask.is_none_or(|key| native.masks.iter().any(|mask| key == &*mask.key))
                        && archive
                            .documents()
                            .iter()
                            .any(|document| document.key() == native.identity.protocol_id())
                })
        })
    }
    /// Validate one saved association through the native archive metadata owner. No graph parsing
    /// or frontend identity can create an archive document, surface or mask with this predicate.
    pub(crate) fn archive_association(
        archive: &InputDocumentArchive,
        workflow: &str,
        input: &sugarsubstitute_contracts::desktop_session::wire::SessionInput,
    ) -> bool {
        metadata::decode(archive.host_metadata()).is_ok_and(|metadata| {
            metadata
                .workflows
                .iter()
                .filter(|(id, _)| &**id == workflow)
                .flat_map(|(_, surfaces)| surfaces)
                .any(|surface| {
                    surface.identity.protocol_id() == input.document_id
                        && archive
                            .documents()
                            .iter()
                            .any(|document| document.key() == input.document_id)
                        && input.surface.as_ref().is_none_or(|key| key == &*surface.key)
                        && input
                            .mask
                            .as_ref()
                            .is_none_or(|key| surface.masks.iter().any(|mask| key == &*mask.key))
                })
        })
    }

    /// Capture actual native documents and their authoritative workflow/surface associations
    /// for a worker-side workspace publication. Pending input mutations are observed first.
    pub(crate) fn workspace_inputs(
        &self,
    ) -> Result<Option<crate::input_documents::workspace::WorkspaceInputs>, ArchiveError> {
        if !self.inputs_restored()
            || self.inputs.running.is_some()
            || !self.inputs.queue.is_empty()
            || self.inputs.route.is_some()
        {
            return Ok(None);
        }
        let archive = self.input_archive()?;
        let documents = archive.documents();
        let surfaces = self
            .inputs
            .workflows
            .iter()
            .flat_map(|(workflow, surfaces)| {
                surfaces
                    .iter()
                    .filter(|surface| {
                        documents
                            .iter()
                            .any(|document| document.key() == surface.identity.protocol_id())
                    })
                    .map(|surface| crate::input_documents::workspace::WorkspaceSurface {
                        workflow: workflow.to_string(),
                        surface: surface.key.to_string(),
                        document: surface.identity.protocol_id(),
                        masks: surface.masks.iter().map(|mask| mask.key.to_string()).collect(),
                    })
            })
            .collect();
        Ok(Some(crate::input_documents::workspace::WorkspaceInputs {
            archive,
            generation: self.inputs.persistence.as_ref().map_or(0, |value| value.generation),
            surfaces,
        }))
    }
    /// Borrow the actual input owner generation for workspace autosave coalescing.
    pub(crate) fn workspace_input_generation(&self) -> Option<u64> {
        self.inputs
            .persistence
            .as_ref()
            .filter(|_| self.inputs_restored())
            .map(|value| value.generation)
    }
}
