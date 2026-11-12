// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide, after each durable save, which retained originals no input document needs.
//!
//! Every surface's base and mask files still in use are kept; the decision runs as one owned
//! job through [`RetainedBases::unreferenced`]. Removing what it selects needs the storage
//! owner's ownership-verified retirement, which this owner does not perform itself; until that
//! exists the selected files are reported and left in place.
use super::*;
use crate::input_documents::bases::RetainedBases;

/// Working bytes of one retirement decision: a bounded listing and small reference records.
const RETIREMENT_BYTES: u64 = 4 * 1024 * 1024;

impl Session {
    /// Submit the retirement decision for the documents as they now are, unless one runs.
    pub(super) fn plan_base_retirement(&mut self) -> Result<(), Box<dyn Error>> {
        if self
            .inputs
            .persistence
            .as_ref()
            .is_none_or(|persistence| persistence.retirement.is_some())
        {
            return Ok(());
        }
        let keep = self.input_references();
        let spec = self.event_spec(
            Operation::Infrastructure,
            JobClass::InfrastructureOrdinary,
            RETIREMENT_BYTES,
            None,
        )?;
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let persistence = self.inputs.persistence.as_mut().ok_or("Input persistence vanished")?;
        let bases = RetainedBases::new(persistence.store.directory());
        match runtime.client.submit_blocking(&runtime.scope, spec, move |context| {
            bases.unreferenced(keep.iter().map(|reference| &**reference), &context)
        }) {
            Ok(job) => persistence.retirement = Some(job),
            // The next durable save decides again.
            Err(cause) if activity::retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }

    /// Observe a finished retirement decision and report what it selected.
    pub(super) fn observe_base_retirement(&mut self) {
        let Some(persistence) = self.inputs.persistence.as_mut() else { return };
        let Some(job) = &mut persistence.retirement else { return };
        let Some(completion) = job.try_take_completion() else { return };
        persistence.retirement = None;
        match completion.outcome {
            JobOutcome::Success(unreferenced) if unreferenced.is_empty() => {}
            JobOutcome::Success(unreferenced) => tracing::info!(
                operation = "input_documents_retained_bases",
                files = unreferenced.len(),
                "Retained input originals no document needs are kept until the storage owner can retire them"
            ),
            JobOutcome::Failure(cause) => tracing::warn!(
                operation = "input_documents_retained_bases",
                cause_chain = %crate::diagnostics::CauseChain(&cause),
                "Retained input originals could not be inventoried"
            ),
            other => tracing::debug!(
                operation = "input_documents_retained_bases",
                outcome = ?other.kind(),
                "The retained-original inventory stopped"
            ),
        }
    }

    /// Every file reference a surface still uses: its base, wanted or loaded, and its masks'
    /// initial files.
    fn input_references(&self) -> Vec<Arc<str>> {
        let mut references = Vec::new();
        for surface in self.inputs.workflows.iter().flat_map(|(_, surfaces)| surfaces) {
            for base in std::iter::once(&surface.desired_base).chain(surface.loaded_base.as_ref()) {
                if let Base::File(reference) = base {
                    references.push(reference.clone());
                }
            }
            for mask in &surface.masks {
                references.extend(mask.desired_initial.iter().chain(&mask.loaded_initial).cloned());
            }
        }
        references.sort();
        references.dedup();
        references
    }
}
