// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind a passive capture once to actual authenticated request and elected run facts.
use super::{reading::PinnedOriginals, *};
use crate::durable_storage::{
    files,
    protected::{directories::Mode, record_file},
    protection, record_publication,
};
use serde::{Deserialize, Serialize};
use sugarsubstitute_contracts::lifecycle::RunKey;
use sugarsubstitute_execution::AuthenticatedEntryLaunch;

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Adoption {
    schema_version: u32,
    manifest: [u8; 32],
    request: u64,
    source_ticks: i64,
    source_frequency: i64,
    recipient_namespace: [u8; 32],
    recipient_epoch: [u8; 16],
    run_instance: u64,
    run_generation: u64,
}
pub(super) fn bind(
    parent: &Directory,
    originals: &PinnedOriginals,
    request: &AuthenticatedEntryLaunch,
    run: RunKey,
    context: &JobContext,
) -> io::Result<std::fs::File> {
    let reference = request
        .view()
        .map_err(io::Error::other)?
        .originals()
        .ok_or_else(|| invalid("Authenticated original reference absent"))?;
    let expected = Adoption {
        schema_version: 1,
        manifest: *reference.manifest_sha256(),
        request: request.request_id().get(),
        source_ticks: request.source_deadline().ticks(),
        source_frequency: request.source_deadline().frequency(),
        recipient_namespace: *run.owner.namespace.bytes(),
        recipient_epoch: *run.owner.epoch.bytes(),
        run_instance: run.instance.get(),
        run_generation: run.generation.get(),
    };
    if originals.manifest.is_none() {
        return Err(invalid("Passive originals have not been validated"));
    }
    bind_record(parent, reference, &expected, context)
}
fn bind_record(
    parent: &Directory,
    reference: EntryOriginalsReference,
    expected: &Adoption,
    context: &JobContext,
) -> io::Result<std::fs::File> {
    let leaf = record_file::hex(reference.manifest_sha256());
    protection::with(context, |owner| {
        let path = parent.path().join("adoptions-v1");
        owner.directory(&path, Mode::OpenOrCreate, context)?;
        let directory = Directory::open(&path, false)?;
        let writer =
            owner.exclusive_rendezvous(&directory.path().join(format!("{leaf}.lock")), context)?;
        let outcome = (|| {
            let name = format!("{leaf}.json");
            let mut file = match directory.read(&name) {
                Ok(file) => file,
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
                    record_publication::publish(
                        &directory,
                        &name,
                        &serde_json::to_vec(expected).map_err(io::Error::other)?,
                        4096,
                        context,
                    )?;
                    directory.read(&name)?
                }
                Err(cause) => return Err(cause),
            };
            let verified = (|| {
                files::single_link(&file)?;
                owner.verify_file(&file)?;
                let bytes = files::read_admitted(&mut file, 4096, context)?;
                let observed: Adoption =
                    serde_json::from_slice(&bytes).map_err(io::Error::other)?;
                if observed != *expected {
                    return Err(invalid(
                        "Passive originals already belong to another authenticated invocation",
                    ));
                }
                Ok(())
            })();
            match verified {
                Ok(()) => Ok(file),
                Err(cause) => finish(Err(cause), owner.close_file(file)),
            }
        })();
        finish(outcome, owner.close_file(writer))
    })
}
fn finish<T>(outcome: io::Result<T>, closed: io::Result<()>) -> io::Result<T> {
    match (outcome, closed) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(cause), Ok(())) | (Ok(_), Err(cause)) => Err(cause),
        (Err(operation), Err(cause)) => Err(io::Error::other(protection::DomainFailure::Native {
            cause,
            operation: Some(operation),
        })),
    }
}
#[cfg(test)]
mod tests;
