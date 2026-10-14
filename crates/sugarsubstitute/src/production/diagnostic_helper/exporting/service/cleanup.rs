// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prehold both genuine cleanup roles and keep their first frozen cutoffs through activation.
use super::*;
impl ExportService {
    /// Reserve genuine source cleanup before either native owner is constructed.
    pub(in crate::production) fn reserve_source_cleanup(
        &mut self,
        client: &ExecutionClient,
        cleanup_scope: &Scope,
        cleanup: JobSpec,
        main_scope: &Scope,
        main: JobSpec,
    ) -> Result<(), ExportIssue> {
        if self.cleanup_admitted[0]
            || self.source_held.is_some()
            || main_scope.owner_id() != self.source.owner()
        {
            return Err(ExportIssue::Identity);
        }
        let source = self.source.alias();
        let cleanup_client = client.clone();
        match client.reserve_symbol_cleanup(
            cleanup_scope,
            cleanup,
            main_scope,
            main,
            move |context| {
                let mut loan = source.loan(false)?;
                let operation = loan.operation()?;
                if let Err(cause) = operation.retire_source(&cleanup_client, &context) {
                    operation.source_failure.get_or_insert(cause);
                    return Ok(false);
                }
                Ok(true)
            },
        ) {
            Ok(permit) => {
                self.source_held = Some(permit);
                self.cleanup_admitted[0] = true;
                Ok(())
            }
            Err(cause) => {
                self.admission.get_or_insert(cause.error);
                Err(ExportIssue::Admission)
            }
        }
    }
    /// Reserve genuine receiving cleanup before either native owner is constructed.
    pub(in crate::production) fn reserve_receiving_cleanup(
        &mut self,
        client: &ExecutionClient,
        cleanup_scope: &Scope,
        cleanup: JobSpec,
        main_scope: &Scope,
        main: JobSpec,
    ) -> Result<(), ExportIssue> {
        if self.cleanup_admitted[1]
            || self.receiving_held.is_some()
            || main_scope.owner_id() != self.source.owner()
        {
            return Err(ExportIssue::Identity);
        }
        let source = self.source.alias();
        let cleanup_client = client.clone();
        match client.reserve_report_export_cleanup(
            cleanup_scope,
            cleanup,
            main_scope,
            main,
            move |context| {
                let mut loan = source.loan(false)?;
                let operation = loan.operation()?;
                if let Err(cause) = operation.retire_receiving(&cleanup_client, &context) {
                    operation.receiving_failure.get_or_insert(cause);
                    return Ok(false);
                }
                Ok(true)
            },
        ) {
            Ok(permit) => {
                self.receiving_held = Some(permit);
                self.cleanup_admitted[1] = true;
                Ok(())
            }
            Err(cause) => {
                self.admission.get_or_insert(cause.error);
                Err(ExportIssue::Admission)
            }
        }
    }
    /// Activate independent cleanup sequentially after both exact original Main deliveries.
    /// Freeze both cutoffs before receiving cleanup starts; source cleanup never renews time.
    pub(in crate::production) fn advance_cleanup(
        &mut self,
        client: &ExecutionClient,
    ) -> Result<bool, ExportIssue> {
        if self.delivery[..2].iter().any(|delivery| delivery.completion.is_none()) {
            return Ok(false);
        }
        if self.source_deadline.is_none()
            && let Some(permit) = self.source_held.as_ref()
        {
            self.source_deadline = Some(
                client
                    .freeze_symbol_cleanup_deadline(permit, std::time::Duration::from_secs(15))
                    .map_err(|cause| {
                    self.admission.get_or_insert(cause);
                    ExportIssue::Admission
                })?,
            );
        }
        if self.receiving_deadline.is_none()
            && let Some(permit) = self.receiving_held.as_ref()
        {
            self.receiving_deadline = Some(
                client
                    .freeze_report_export_cleanup_deadline(
                        permit,
                        std::time::Duration::from_secs(15),
                    )
                    .map_err(|cause| {
                        self.admission.get_or_insert(cause);
                        ExportIssue::Admission
                    })?,
            );
        }
        if self.delivery[2].job.is_none() && self.delivery[2].completion.is_none() {
            let Some(permit) = self.receiving_held.take() else {
                return Ok(false);
            };
            let cutoff = self.receiving_deadline.ok_or(ExportIssue::Identity)?;
            match permit.begin(cutoff) {
                Ok(job) => {
                    self.delivery[2].job = Some(job);
                    return Ok(true);
                }
                Err(rejection) => {
                    self.receiving_held = Some(rejection.permit);
                    self.admission.get_or_insert(rejection.error);
                    return Err(ExportIssue::Admission);
                }
            }
        }
        if self.delivery[2].completion.is_some()
            && self.delivery[3].job.is_none()
            && self.delivery[3].completion.is_none()
        {
            let Some(permit) = self.source_held.take() else {
                return Ok(false);
            };
            let cutoff = self.source_deadline.ok_or(ExportIssue::Identity)?;
            match permit.begin(cutoff) {
                Ok(job) => {
                    self.delivery[3].job = Some(job);
                    return Ok(true);
                }
                Err(rejection) => {
                    self.source_held = Some(rejection.permit);
                    self.admission.get_or_insert(rejection.error);
                    return Err(ExportIssue::Admission);
                }
            }
        }
        Ok(false)
    }
}
