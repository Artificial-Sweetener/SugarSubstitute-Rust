// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reproject the sealed registered body on the same bounded finite owner.
use super::*;
use sugarsubstitute_contracts::{coordinator_entry::EntrySetupBudget, lifecycle::RunKey};
/// Inert pending inputs retain the genuine original source and the first restart interval.
pub(in crate::production::entry_activation) struct RestartProjection {
    pub(in crate::production::entry_activation) source: RegisteredStartupSource,
    pub(in crate::production::entry_activation) previous: RunKey,
    pub(in crate::production::entry_activation) run: RunKey,
    pub(in crate::production::entry_activation) original: EntrySetupBudget,
    pub(in crate::production::entry_activation) connection: EntryConnectionId,
    pub(in crate::production::entry_activation) request: EntryRequestId,
}
impl ProjectionInput {
    pub(in crate::production::entry_activation) fn restart(
        client: ExecutionClient,
        capture_root: PathBuf,
        restart: RestartProjection,
    ) -> Self {
        Self {
            request: None,
            snapshot: None,
            client: Some(client),
            run: Some(restart.run),
            restart: Some(restart),
            capture_root,
            cause: ProjectionCause::NotStarted,
            post: None,
            _charge: None,
        }
    }
}
pub(super) fn project(
    mut input: ChargedBlockingInput<ProjectionInput>,
    context: JobContext,
) -> Result<ProjectedEntry, ProjectionFailure> {
    let result = (|| -> Result<_, Box<dyn Error + Send + Sync>> {
        let data = input.input().restart.as_ref().ok_or("Registered restart input missing")?;
        check(&context, data.original.started(), data.original.until())?;
        let fields = fields(data.source.view()?)?;
        let client =
            input.input().client.as_ref().ok_or("Actual restart execution owner missing")?;
        let snapshot = client.restart_startup_snapshot(
            &data.source,
            data.previous,
            data.run,
            data.original,
            &context,
        )?;
        check(&context, data.original.started(), data.original.until())?;
        Ok((fields, snapshot))
    })();
    let (fields, snapshot) = match result {
        Ok(value) => value,
        Err(cause) => {
            input.input_mut().cause = ProjectionCause::Operation(cause);
            return Err(ProjectionFailure { retained: input });
        }
    };
    let Some(data) = input.input_mut().restart.take() else {
        input.input_mut().snapshot = Some(snapshot);
        return Err(ProjectionFailure { retained: input });
    };
    let Some(charge) = input.input_mut()._charge.take() else {
        input.input_mut().snapshot = Some(snapshot);
        input.input_mut().restart = Some(data);
        return Err(ProjectionFailure { retained: input });
    };
    Ok(ProjectedEntry {
        locale: fields.locale,
        snapshot: Some(snapshot),
        registered: data.source,
        until: data.original.until(),
        entry_connection: data.connection,
        entry_request: data.request,
        capture_root: std::mem::take(&mut input.input_mut().capture_root),
        requested_profile: fields.requested_profile,
        profile_units: fields.profile_units,
        profile: fields.profile,
        target: fields.target,
        frontend: Some(fields.frontend),
        backend_environment: fields.backend_environment,
        desktop_environment: Some(fields.desktop_environment),
        _charge: charge,
    })
}
