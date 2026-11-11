// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode complete current/historical identities without epoch rebasing.
use super::{DiagnosticWireError as E, reader::Reader, writer::Writer};
use sugarsubstitute_contracts::{incident::*, lifecycle::*};
pub(super) fn owner(w: &mut Writer<'_>, key: CoordinatorKey) -> Result<(), E> {
    w.put(key.namespace.bytes())?;
    w.put(key.epoch.bytes())
}
pub(super) fn read_owner(r: &mut Reader<'_>) -> Result<CoordinatorKey, E> {
    Ok(CoordinatorKey {
        namespace: NamespaceId::new(r.array()?),
        epoch: CoordinatorEpoch::new(r.array()?).map_err(|_| E::Identity)?,
    })
}
pub(super) fn key(w: &mut Writer<'_>, key: DiagnosticKey) -> Result<(), E> {
    owner(w, key.owner())?;
    w.u8(match key.role() {
        DiagnosticRole::Storage => 1,
        DiagnosticRole::Symbolize => 2,
        DiagnosticRole::Reporter => 3,
    })?;
    w.u64(key.session().get())
}
pub(super) fn read_key(r: &mut Reader<'_>) -> Result<DiagnosticKey, E> {
    let owner = read_owner(r)?;
    let role = match r.u8()? {
        1 => DiagnosticRole::Storage,
        2 => DiagnosticRole::Symbolize,
        3 => DiagnosticRole::Reporter,
        _ => return Err(E::Role),
    };
    Ok(DiagnosticKey::new(
        owner,
        role,
        DiagnosticSessionId::new(r.u64()?).map_err(|_| E::Identity)?,
    ))
}
pub(super) fn incident(w: &mut Writer<'_>, key: IncidentKey) -> Result<(), E> {
    match key.subject() {
        IncidentSubject::Supervisor(key) => {
            w.u8(1)?;
            owner(w, key)
        }
        IncidentSubject::Desktop(run) => {
            w.u8(2)?;
            owner(w, run.owner)?;
            w.u64(run.instance.get())?;
            w.u64(run.generation.get())
        }
    }
}
pub(super) fn read_incident(r: &mut Reader<'_>) -> Result<IncidentKey, E> {
    let tag = r.u8()?;
    let key = read_owner(r)?;
    Ok(IncidentKey::new(match tag {
        1 => IncidentSubject::Supervisor(key),
        2 => IncidentSubject::Desktop(RunKey {
            owner: key,
            instance: InstanceId::new(r.u64()?).map_err(|_| E::Identity)?,
            generation: RunGeneration::new(r.u64()?).map_err(|_| E::Identity)?,
        }),
        _ => return Err(E::Schema),
    }))
}
