// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Build inert matching facts without native capabilities.
use super::super::*;
use sugarsubstitute_contracts::{incident::*, lifecycle::*};
pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;
pub(super) fn owner(epoch: u8) -> Result<CoordinatorKey, IdentityError> {
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([9; 32]),
        epoch: CoordinatorEpoch::new([epoch; 16])?,
    })
}
pub(super) fn key(role: DiagnosticRole) -> Result<DiagnosticKey, Box<dyn std::error::Error>> {
    Ok(DiagnosticKey::new(owner(1)?, role, DiagnosticSessionId::new(7)?))
}
pub(super) fn subject() -> Result<DiagnosticIncident, Box<dyn std::error::Error>> {
    Ok(DiagnosticIncident {
        key: IncidentKey::new(IncidentSubject::Desktop(RunKey {
            owner: owner(2)?,
            instance: InstanceId::new(3)?,
            generation: RunGeneration::new(4)?,
        })),
        revision: IncidentRevision::new(5)?,
    })
}
pub(super) fn binding() -> Result<ReportBinding, Box<dyn std::error::Error>> {
    let i = subject()?;
    Ok(ReportBinding::new(i.key, i.revision, ReportSessionId::new(8)?))
}
pub(super) fn storage<'a>(
    id: u64,
    command: DiagnosticCommand<'a>,
) -> Result<DiagnosticFrame<'a>, Box<dyn std::error::Error>> {
    Ok(DiagnosticFrame {
        key: key(DiagnosticRole::Storage)?,
        request: DiagnosticRequestId::new(id)?,
        context: DiagnosticContext {
            operation: Some(LogicalStoreOperationKey::new(owner(1)?, IncidentOperationId::new(6)?)),
            incident: Some(subject()?),
            deadline: Some(DiagnosticDeadline::new(100, 10)?),
            at: Some(IncidentUtc::from_unix_seconds(20)?),
        },
        command,
    })
}
pub(super) fn reporter<'a>(
    id: u64,
    command: DiagnosticCommand<'a>,
    finite: bool,
) -> Result<DiagnosticFrame<'a>, Box<dyn std::error::Error>> {
    Ok(DiagnosticFrame {
        key: key(DiagnosticRole::Reporter)?,
        request: DiagnosticRequestId::new(id)?,
        context: DiagnosticContext {
            incident: Some(subject()?),
            deadline: if finite { Some(DiagnosticDeadline::new(100, 10)?) } else { None },
            ..DiagnosticContext::default()
        },
        command,
    })
}
pub(super) fn encoded(frame: DiagnosticFrame<'_>) -> Result<Vec<u8>, DiagnosticWireError> {
    let mut bytes = vec![0; MAX_FRAME_BYTES];
    let length = encode(frame, &mut bytes)?;
    bytes.truncate(length);
    Ok(bytes)
}
pub(super) fn issued(
    router: &mut DiagnosticRequestRouter,
    bytes: &[u8],
) -> Result<IssuedDiagnosticRequest, Box<dyn std::error::Error>> {
    match router.admit(bytes)? {
        DiagnosticAdmission::Execute(token) => Ok(token),
        _ => Err("Expected first issued request".into()),
    }
}
