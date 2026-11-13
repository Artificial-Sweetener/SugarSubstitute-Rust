// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stage whole failure/source custody before fallible strict record retirement.
use super::*;
use sugarsubstitute_supervision::WindowsProcessRegistry;
impl ReporterSession {
    /// Retain the actual original pipe/source and moved error across every failed removal attempt.
    /// Only actual registered family removal exposes ReporterRetired; failure always vetoes reuse.
    pub(in crate::production::entry_activation::incident_reporting) fn retire_original_family(
        &mut self,
        registry: &mut WindowsProcessRegistry,
        binding: ReportBinding,
        expected: DiagnosticKey,
        source: &ReporterSource,
        process: ProcessKey,
    ) -> Result<()> {
        let facts = source.poll_retiring()?;
        if expected.owner().namespace != binding.incident().subject().coordinator().namespace
            || expected.role() != DiagnosticRole::Reporter
            || expected.session().get() != binding.session().get()
        {
            return Err("Actual current Reporter key differs from historical data".into());
        }
        let actual = registry.snapshot(process)?;
        if facts.process != Some(process)
            || !facts.pipe_settled
            || !actual.settled
            || actual.binding != sugarsubstitute_supervision::FamilyBinding::Diagnostic(expected)
        {
            return Err("Actual checked original Reporter retirement changed".into());
        }
        self.preparing()?;
        if self.retirement.is_none() {
            self.retirement = Some(ReporterRetirement::Original(OriginalReporterRetirement {
                binding,
                _source: source.alias(),
                _process: process,
                family: None,
                registered: false,
            }));
        }
        let Some(ReporterRetirement::Original(original)) = &mut self.retirement else {
            return Err("Original Reporter retirement slot changed".into());
        };
        if original.binding != binding
            || original._process != process
            || !original._source.same_allocation(source)
            || original.registered
        {
            return Err("Actual original Reporter retirement correlation changed".into());
        }
        if original.family.is_none() {
            original.family = registry.take_settled_failure(process)?;
        }
        while registry.take_observation(process)?.is_some() {}
        registry.retire_settled(process)?;
        original.registered = true;
        Ok(())
    }
}
