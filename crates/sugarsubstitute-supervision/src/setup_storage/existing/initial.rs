// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resume first setup while preserving protected unfinished predecessors from supersession.
use super::*;
use crate::setup_storage::SelectionStore;
use crate::setup_storage::transaction::generation;
use sugarsubstitute_contracts::installation::{Candidate, GenerationId};

/// Distinguish interrupted fresh setup from damaged installed configuration without an error fallback.
/// Completed or unknown generations require ordinary recovery; their saved settings must not reset.
/// Protected unfinished predecessors remain on disk. Only the selected candidate can be resumed
/// by the independently authenticated preparation path; inspection grants no payload authority.
pub(super) fn incomplete_candidate(root: &Directory, context: &JobContext) -> io::Result<bool> {
    let Some(store) = SelectionStore::existing(&root.path, context).map_err(io::Error::other)?
    else {
        return Ok(false);
    };
    let record = store
        .load(context)
        .map_err(io::Error::other)?
        .ok_or_else(|| invalid("Installation selection is absent"))?;
    if record.accepted().is_some() || record.retained().is_some() {
        return Ok(false);
    }
    let Some(Candidate::Staging { generation }) = record.candidate() else {
        return Ok(false);
    };
    let releases = match Directory::open(&root.path.join("releases"), false) {
        Ok(directory) => directory,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(true),
        Err(cause) => return Err(cause),
    };
    for (index, entry) in std::fs::read_dir(&releases.path)?.enumerate() {
        check(context)?;
        if index >= 1024 {
            return Err(invalid("Initial generation inventory exceeds its bound"));
        }
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Ok(false);
        };
        if name == generation.id.as_str() {
            if !generation::unconfigured(&entry.path(), Some(generation.package.as_str()), context)?
            {
                return Ok(false);
            }
        } else if name.starts_with(generation::INITIAL_PREFIX) {
            // Private allocation may have stopped before its marker was written. Pin ancestry
            // to reject links; these directories are preserved and never become candidates.
            let unfinished = Directory::open(&entry.path(), false)?;
            match unfinished.read("installation-ready.json") {
                Ok(_) => return Ok(false),
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
                Err(cause) => return Err(cause),
            }
        } else if GenerationId::new(name.to_owned()).is_err()
            || !generation::unconfigured(&entry.path(), None, context)?
        {
            return Ok(false);
        }
    }
    check(context)?;
    Ok(true)
}
