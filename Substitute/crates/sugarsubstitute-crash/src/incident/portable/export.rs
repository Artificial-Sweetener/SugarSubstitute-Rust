// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Export original attachment bytes through the retained passive reader and caller-owned sink.
use super::*;
use std::io::Write;

impl PortableIncidentHistory<'_> {
    /// Stream one allowlisted committed attachment into an already-owned export sink.
    ///
    /// The caller owns destination permissions and atomic final publication. Success is
    /// returned only after exact complete hash/length and current source revalidation.
    /// A failed sink may contain a partial output; failure never acknowledges the incident.
    /// The supplied context must be this view's original finite operation. A later
    /// request opens and qualifies a fresh source session instead of renewing this view.
    /// # Errors
    /// Preserves absent/foreign selection, changed source, corruption, sink and job causes.
    pub fn export_attachment(
        &mut self,
        incident: IncidentKey,
        kind: AttachmentKind,
        output: &mut dyn Write,
        context: &JobContext,
    ) -> Result<PortableIncidentExport, PortableIncidentError> {
        source::control(context)?;
        if !crate::incident::publication::same_context(&self.context, context) {
            return Err(source::rejected(PortableIncidentFailureCode::ChangedSource));
        }
        let memory = context
            .memory()
            .claim(DIAGNOSTIC_IO_BYTES as u64)
            .map_err(PortableIncidentError::memory)?;
        let result = self.export_current(incident, kind, output, context);
        result.map_err(|cause| cause.retain(memory))
    }
    fn export_current(
        &mut self,
        incident: IncidentKey,
        kind: AttachmentKind,
        output: &mut dyn Write,
        context: &JobContext,
    ) -> Result<PortableIncidentExport, PortableIncidentError> {
        source::control(context)?;
        if incident.subject().coordinator().namespace != self.provenance.namespace() {
            return Err(source::rejected(PortableIncidentFailureCode::ForeignNamespace));
        }
        self.session.revalidate(context)?;
        if self.session.provenance() != self.provenance {
            return Err(source::rejected(PortableIncidentFailureCode::ChangedSource));
        }
        let payload = self.data.payloads.iter().find(|payload| {
            matches!(payload.object, DiagnosticObject::Payload(key, _, actual) if key == incident && actual == kind)
        }).ok_or_else(|| source::rejected(PortableIncidentFailureCode::MissingContent))?;
        source::bind(self.session, payload.entry, payload.object, context)?;
        let hash = source::stream(self.session, payload.entry, context, |chunk| {
            output.write_all(chunk).map_err(PortableIncidentError::source)
        })?;
        if hash != payload.sha256 {
            return Err(source::rejected(PortableIncidentFailureCode::WrongDigest));
        }
        self.session.revalidate(context)?;
        if self.session.provenance() != self.provenance {
            return Err(source::rejected(PortableIncidentFailureCode::ChangedSource));
        }
        Ok(PortableIncidentExport { incident, kind, bytes: payload.entry.bytes, sha256: hash })
    }
}
