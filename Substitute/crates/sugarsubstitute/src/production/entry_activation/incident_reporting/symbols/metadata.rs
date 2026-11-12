// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer only a genuine closed observation into a distinct guarded metadata CAS.
use super::*;
impl IncidentReporting {
    /// Complete the distinct metadata lane only after exact genuine Host adoption and release.
    pub(in crate::production::entry_activation::incident_reporting) fn adopt_symbols_store(
        &mut self,
        host: &mut IncidentHost,
    ) -> Result<()> {
        let successor = self
            .symbols
            .as_mut()
            .ok_or("Actual Symbol receiving owner missing")?
            .adopt_metadata(host)?;
        self.retained_effect = Some(successor);
        self.store = None;
        Ok(())
    }
}
impl SymbolReceiving {
    /// Borrow original report binding while its complete effect remains retained.
    pub(super) const fn binding(&self) -> ReportBinding {
        self.binding
    }
    /// Admit mutation only after genuine provider/reader/selected cleanup produced Verified.
    /// Failure before this point preserves the original Launch without a pending mutation.
    pub(super) fn admit_metadata(
        &mut self,
        host: &mut IncidentHost,
        original: IncidentRequestContext,
    ) -> Result<Option<IncidentOperationScope>> {
        if self.metadata_transferred || self.observation.is_none() {
            return Ok(None);
        }
        if original.budget() != self.read_scope.original() {
            return Err("Original Symbol metadata interval changed".into());
        }
        if self.metadata_scope.is_none() {
            let binding = self.binding();
            self.metadata_scope = Some(host.admit_report_symbols(binding, original)?);
        }
        Ok(self.metadata_scope)
    }
    /// Move the entire sealed observation into real protected work; its source/charge never split.
    pub(super) fn take_metadata_input(
        &mut self,
        host: &mut IncidentHost,
    ) -> Result<Option<ReportingInput>> {
        if self.metadata_transferred {
            return Ok(None);
        }
        let Some(scope) = self.metadata_scope else { return Ok(None) };
        let binding = self.binding();
        let request = host.namespace()?.diagnostic_write_request(scope.operation())?;
        let work = host.reporting_service()?.prepare_symbols(binding, &mut self.observation)?;
        self.metadata_transferred = true;
        Ok(Some(ReportingInput {
            request,
            operation: scope.operation(),
            work,
            authentication: None,
        }))
    }
    /// Adopt only the real CAS proof after actual Core delivery and independently checked cleanup.
    /// The emitted next Launch keeps the same original incident/session and exact successor revision.
    pub(super) fn adopt_metadata(&mut self, host: &mut IncidentHost) -> Result<IncidentEffect> {
        let scope = self.metadata_scope.ok_or("Original Symbol mutation scope missing")?;
        let binding = self.binding();
        host.adopt_report_symbols(scope, binding)?;
        let expected =
            ReportBinding::new(binding.incident(), binding.revision().next()?, binding.session());
        match host.next_effect()? {
            Some(IncidentEffect::LaunchReporter { binding: current, .. })
                if *current == expected => {}
            _ => return Err("Actual adopted Symbol report successor missing".into()),
        }
        host.reporting_service()?.release_adopted()?;
        let successor = host.take_effect()?.ok_or("Actual Symbol successor already consumed")?;
        self.metadata_retired = true;
        self.ready = true;
        Ok(successor)
    }
    /// Record only the actual installed metadata owner's checked close during abnormal shutdown.
    /// A passive restoration requires a distinct checked marker and original retained input below.
    pub(super) fn observe_metadata_retirement(&mut self, service: &ReportingService) {
        if self.metadata_transferred && service.outcome().is_some() && service.files_retired() {
            self.metadata_retired = true;
        }
    }
    /// Re-adopt only the identical passively restored Symbols input with its genuine observation.
    /// The service retains any real completion/cause; this grants no file or CAS adoption authority.
    pub(super) fn restore_metadata(
        &mut self,
        service: &ReportingService,
        input: &mut Option<ReportingInput>,
    ) -> Result<()> {
        let scope = self.metadata_scope.ok_or("Original passive Symbol scope missing")?;
        let marker: PassiveReportingRestoration =
            service.passive_restoration().ok_or("Checked passive Symbol restoration missing")?;
        let (owner, runtime_scope, until) = marker.original();
        let binding = self.binding();
        let value = input.as_ref().ok_or("Original restored Symbol input missing")?;
        let ReportingWork::Symbols { binding: actual, result } = &value.work else {
            return Err("Passive Symbol input changed kind".into());
        };
        let (original_owner, original_until) = result.observation().original_admission()?;
        if !marker.is_symbols()
            || value.operation != scope.operation()
            || *actual != binding
            || marker.operation() != scope.operation()
            || marker.binding() != binding
            || owner != original_owner
            || until != scope.original().until()
            || self.runtime != Some((owner, runtime_scope))
            || original_until > until
            || value.authentication.is_some()
        {
            return Err("Passive Symbol input changed origin".into());
        }
        let value = input.take().ok_or("Original passive Symbol input already consumed")?;
        let ReportingWork::Symbols { result, .. } = value.work else {
            return Err("Original passive Symbol kind changed".into());
        };
        self.observation = Some(result.restore());
        self.metadata_retired = true;
        Ok(())
    }
}
