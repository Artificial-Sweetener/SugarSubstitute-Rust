// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve source-host correlation without adopting an installation or backend resource.
use crate::{
    durable_storage::{
        check,
        files::{self, Directory},
        invalid,
        protected::directories::Mode,
        protection, staging,
    },
    setup_storage::RunningApplication,
};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt,
    io::{self, Write},
    path::Path,
};
use sugarsubstitute_contracts::{
    configuration::ProfileId, execution::JobContext, installation::InstallationId,
};
const RECORD: &str = "source-host-identity.json";
const LOCK: &str = ".source-host-identity.lock";
const MAX_BYTES: usize = 4096;
/// Preserved correlation bound to one native profile directory and the selected profile.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Record {
    version: u32,
    installation: InstallationId,
    profile: String,
    directory: [u32; 3],
}

/// Obtain the existing installed identity, or protected correlation for a legitimate source host.
///
/// Call only after the ordinary launch owner has settled provenance and selected user/profile
/// storage. `None` means that owner actually admitted a source/raw/earlier host; corrupt or
/// contradictory installed selection must fail before this function is called. This never
/// initializes SelectionStore, creates an installation marker or authorizes managed data.
///
/// # Errors
/// Preserves every invalid original; rejects future/foreign/replaced records, redirected paths,
/// competing writers, original job controls and checked native publication/retirement failures.
pub fn session_host_identity(
    user: &Path,
    profile: &ProfileId,
    installed: Option<&RunningApplication>,
    context: &JobContext,
) -> io::Result<InstallationId> {
    check(context)?;
    if let Some(installed) = installed {
        return Ok(installed.installation_id().clone());
    }
    protection::with(context, |protection| {
        let mut path = user.to_owned();
        protection.directory(&path, Mode::OpenOrCreate, context)?;
        for component in ["sessions", profile.as_str()] {
            path.push(component);
            protection.directory(&path, Mode::OpenOrCreate, context)?;
        }
        let directory = Directory::open(&path, false)?;
        let identity = directory.identity()?;
        let expected_directory = [identity.0, identity.1, identity.2];
        let lock = protection.exclusive_rendezvous(&path.join(LOCK), context)?;
        let result = (|| match directory.read(RECORD) {
            Ok(mut file) => {
                let read = (|| {
                    protection.verify_file(&file)?;
                    files::single_link(&file)?;
                    let bytes = files::read_admitted(&mut file, MAX_BYTES, context)?;
                    let record: Record =
                        serde_json::from_slice(&bytes).map_err(io::Error::other)?;
                    if record.version != 1
                        || record.profile != profile.as_str()
                        || record.directory != expected_directory
                    {
                        return Err(invalid(
                            "Source host identity is incompatible with its original profile",
                        ));
                    }
                    Ok(record.installation)
                })();
                finish(read, protection.close_file(file))
            }
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
                let installation =
                    InstallationId::new(staging::identity()?).map_err(io::Error::other)?;
                let record = Record {
                    version: 1,
                    installation: installation.clone(),
                    profile: profile.as_str().into(),
                    directory: expected_directory,
                };
                let bytes = serde_json::to_vec(&record).map_err(io::Error::other)?;
                let stage = staging::unique_path(&directory, ".source-host-identity-pending")?;
                let mut file = protection.file(&stage, context)?;
                let write = file
                    .write_all(&bytes)
                    .and_then(|()| files::sync(&file, context))
                    .and_then(|()| files::file_identity(&file));
                let identity = finish(write, protection.close_file(file))?;
                protection.publish(
                    &stage,
                    &path.join(RECORD),
                    identity,
                    protection::Publication::File { replace: false },
                    context,
                )?;
                check(context)?;
                Ok(installation)
            }
            Err(cause) => Err(cause),
        })();
        finish(result, protection.close_file(lock))
    })
}
/// Retire the native handle while retaining both operation and retirement failures.
fn finish<T>(result: io::Result<T>, retired: io::Result<()>) -> io::Result<T> {
    match (result, retired) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(cause), Ok(())) | (Ok(_), Err(cause)) => Err(cause),
        (Err(original), Err(retirement)) => {
            Err(io::Error::other(Retirement { original, retirement }))
        }
    }
}
/// Preserve the operation cause when checked native retirement also fails.
#[derive(Debug)]
struct Retirement {
    original: io::Error,
    retirement: io::Error,
}
impl fmt::Display for Retirement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Source identity operation and checked retirement failed: {}",
            self.retirement
        )
    }
}
impl Error for Retirement {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.original)
    }
}

#[cfg(test)]
mod tests;
