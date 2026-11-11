// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Perform real selection, protected parsing and native resolution outside metadata locks.
use super::{
    cause::SymbolReportCause as Cause,
    source::{Data, DataOwner, Parking, Source},
};
use crate::incident::{
    DbgHelpSymbolProvider, IncidentDumpReader, IncidentRepository, SystemIncidentUtc,
};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_supervision::{
    InstalledReleaseOwner, committed_reader::CommittedDiagnosticReader,
    installed_release::SymbolBundleOwner,
};
pub(super) fn run(source: Source, client: ExecutionClient, context: JobContext) -> Result<(), ()> {
    source.update(|state| state.started = true);
    let admission = client
        .symbol_provider_admission(&context)
        .map_err(|cause| source.fail(Cause::Admission(cause)))?;
    let provider = match DbgHelpSymbolProvider::new(admission) {
        Ok(owner) | Err(owner) => owner,
    };
    let original = provider.original_origin().cloned().ok_or_else(|| source.fail(Cause::State))?;
    let mut parked = Parking::new(&source, DataOwner::new(provider, original));
    let request = source
        .with(|state| state.request.take())
        .flatten()
        .ok_or_else(|| source.fail(Cause::State))?;
    source.update(|state| state.before_effects = false);
    let data = parked.data.as_mut().ok_or_else(|| source.fail(Cause::State))?;
    let result = resolve(data, client, context, request);
    if let Err(cause) = result {
        data.fail(cause);
        return Err(());
    }
    Ok(())
}
fn resolve(
    data: &mut Data,
    client: ExecutionClient,
    context: JobContext,
    request: super::SymbolReportRequest,
) -> Result<(), Cause> {
    if data.provider.as_ref().is_none_or(|provider| {
        provider.construction_failure().is_some() || provider.failure().is_some()
    }) {
        return Err(Cause::State);
    }
    let admission = client.symbol_bundle_admission(&context).map_err(Cause::Admission)?;
    let owner = SymbolBundleOwner::new(InstalledReleaseOwner::new().symbol_selection(), admission)
        .map_err(|failure| Cause::Native(failure.cause))?;
    data.selected_owner = Some(owner);
    data.selected_owner.as_mut().ok_or(Cause::State)?.select().map_err(Cause::Native)?;
    match data.selected_owner.take().ok_or(Cause::State)?.take_verified() {
        Ok(lease) => data.selected = Some(lease),
        Err(owner) => {
            data.selected_owner = Some(owner);
            return Err(Cause::State);
        }
    }
    let admission = client.committed_reader_admission(&context).map_err(Cause::Admission)?;
    let reader = CommittedDiagnosticReader::new(request.protected, admission)
        .map_err(|failure| Cause::Native(failure.cause))?;
    data.native_reader = Some(reader);
    match IncidentDumpReader::new(data.native_reader.take().ok_or(Cause::State)?) {
        Ok(reader) => data.reader = Some(reader),
        Err(failure) => {
            data.native_reader = Some(failure.reader);
            return Err(Cause::Decode(failure.cause));
        }
    }
    let reader = data.reader.as_mut().ok_or(Cause::State)?;
    let utc = SystemIncidentUtc
        .sample(reader.original_origin().ok_or(Cause::State)?)
        .map_err(Cause::Utc)?;
    IncidentRepository::open_dump(reader, request.key, request.revision, request.dump, utc)
        .map_err(Cause::Dump)?;
    match data.reader.take().ok_or(Cause::State)?.take_verified() {
        Ok(lease) => data.dump = Some(lease),
        Err(reader) => {
            data.reader = Some(reader);
            return Err(Cause::State);
        }
    }
    let dump = data.dump.as_mut().ok_or(Cause::State)?;
    let frame = dump.exception_frame().map_err(Cause::Frame)?;
    data.observation = Some(
        data.provider
            .as_mut()
            .ok_or(Cause::State)?
            .resolve(data.selected.as_ref().ok_or(Cause::State)?, frame, request.dump)
            .map_err(Cause::Provider)?,
    );
    // Actual final observation happens while Main is Running and all pins/fence remain held.
    // Paired cleanup is close-only and cannot fabricate fresh read authority.
    dump.check_current().map_err(Cause::Dump)?;
    Ok(())
}
pub(super) fn cleanup(
    source: Source,
    client: ExecutionClient,
    context: JobContext,
) -> Result<(), ()> {
    let Some(data) = source.with(|state| state.data.take()).flatten() else {
        if source.with(|state| state.before_effects) == Some(true) {
            source.update(|state| state.settled = true);
            return Ok(());
        }
        source.fail(Cause::State);
        return Err(());
    };
    let mut parked = Parking::new(&source, data);
    let data = parked.data.as_mut().ok_or_else(|| source.fail(Cause::State))?;
    if let Err(cause) = close(data, &client, &context) {
        data.retirement = Some(cause);
        return Err(());
    }
    data.settled = true;
    source.update(|state| state.settled = true);
    Ok(())
}
fn close(data: &mut Data, client: &ExecutionClient, context: &JobContext) -> Result<(), Cause> {
    if let Some(provider) = data.provider.as_ref() {
        let proof = client
            .symbol_provider_retirement(provider.original_origin().ok_or(Cause::State)?, context)
            .map_err(Cause::Admission)?;
        data.provider
            .as_mut()
            .ok_or(Cause::State)?
            .retire_in_place(&proof)
            .map_err(Cause::Provider)?;
    }
    if let Some(origin) = data
        .dump
        .as_ref()
        .and_then(|dump| dump.original_origin())
        .or_else(|| data.reader.as_ref().and_then(|reader| reader.original_origin()))
        .or_else(|| data.native_reader.as_ref().map(CommittedDiagnosticReader::original_origin))
    {
        let proof =
            client.committed_reader_retirement(origin, context).map_err(Cause::Admission)?;
        if let Some(dump) = data.dump.as_mut() {
            dump.retire_in_place(&proof).map_err(Cause::Dump)?;
        }
        if let Some(reader) = data.reader.as_mut() {
            reader.retire_in_place(&proof).map_err(Cause::Dump)?;
        }
        if let Some(reader) = data.native_reader.take()
            && let Err(failure) = reader.retire(&proof)
        {
            data.native_reader = Some(failure.owner);
            return Err(Cause::Native(failure.cause));
        }
    }
    if let Some(origin) = data
        .selected
        .as_ref()
        .map(|lease| lease.original_origin())
        .or_else(|| data.selected_owner.as_ref().map(|owner| owner.original_origin()))
    {
        let proof = client.symbol_bundle_retirement(origin, context).map_err(Cause::Admission)?;
        let result = if let Some(lease) = data.selected.take() {
            lease.retire(&proof)
        } else {
            data.selected_owner.take().ok_or(Cause::State)?.retire(&proof)
        };
        if let Err(failure) = result {
            data.selected_owner = Some(failure.owner);
            return Err(Cause::Native(failure.cause));
        }
    }
    Ok(())
}
