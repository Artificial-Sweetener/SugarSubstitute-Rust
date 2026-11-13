// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recover exact launcher executable and ownership replacement under the installation writer.
use super::*;
use serde::{Deserialize, Serialize};

const JOURNAL: &str = ".launcher-transition-v1.json";
const LIMIT: usize = 64 * 1024;

#[cfg(test)]
mod tests;

/// Both identities are durable before replacing the executable; ownership advances afterward.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Transition {
    schema_version: u32,
    installation_directory: (u32, u32, u32),
    previous: record::Record,
    next: record::Record,
}

/// Record a compatible authenticated replacement before touching the root executable.
pub(super) fn begin(
    root: &Directory,
    state: &Directory,
    previous: record::Record,
    next: record::Record,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<record::Record> {
    match root.read(ENTRY) {
        Ok(mut file) => {
            protection.verify_file(&file)?;
            payload::verify(&mut file, &previous, context)?;
        }
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
        Err(cause) => return Err(cause),
    }
    let transition =
        Transition { schema_version: 1, installation_directory: root.identity()?, previous, next };
    validate(&transition)?;
    let bytes = serde_json::to_vec(&transition).map_err(io::Error::other)?;
    if bytes.len() > LIMIT {
        return Err(invalid("Launcher transition exceeds its bound"));
    }
    publication::write_immutable(state, JOURNAL, &bytes, protection, context)?;
    recover(root, state, protection, context)?
        .ok_or_else(|| invalid("Published launcher transition is absent"))
}

/// Resume either side of executable publication without treating unknown bytes as ours.
pub(super) fn recover(
    root: &Directory,
    state: &Directory,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<Option<record::Record>> {
    let Some(pending) = pending(root, state, protection, context)? else { return Ok(None) };
    let Pending { transition, identity, current, next_present } = pending;
    let _image = if next_present {
        payload::ensure(root, state, &transition.next, protection, context)?
    } else {
        let replacing = files::exists(root.path().join(ENTRY), context)?;
        payload::replace(root, state, &transition.next, replacing, protection, context)?
    };
    if current != transition.next {
        record::replace(state, &current, &transition.next, protection, context)?;
    }
    check(context)?;
    protection.remove_journaled(&state.path().join(JOURNAL), identity, context)?;
    Ok(Some(transition.next))
}

/// Replace a pending intent using a freshly authenticated, pinned candidate under the writer.
/// Reconcile ownership to the image already published before retiring the old intent. This does
/// not require the old repair sources. An interruption leaves either the old recoverable intent,
/// consistent ownership, or the new durable intent; unknown images are never adopted.
pub(super) fn supersede(
    root: &Directory,
    state: &Directory,
    next: record::Record,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<record::Record> {
    if let Some(pending) = pending(root, state, protection, context)? {
        require_replacement(&pending.transition.previous, &next)?;
        require_replacement(&pending.transition.next, &next)?;
        if pending.next_present && pending.current != pending.transition.next {
            record::replace(
                state,
                &pending.current,
                &pending.transition.next,
                protection,
                context,
            )?;
        }
        check(context)?;
        protection.remove_journaled(&state.path().join(JOURNAL), pending.identity, context)?;
    }
    let previous = record::read(state, protection, context)?
        .ok_or_else(|| invalid("Launcher replacement has lost established ownership"))?;
    if previous == next {
        return Ok(previous);
    }
    begin(root, state, previous, next, protection, context)
}

/// Validated intent and observed publication boundary, read without changing either source.
struct Pending {
    transition: Transition,
    identity: (u32, u32, u32),
    current: record::Record,
    next_present: bool,
}

/// Validate the native installation binding, ownership and image before any recovery mutation.
fn pending(
    root: &Directory,
    state: &Directory,
    protection: &Protection,
    context: &JobContext,
) -> io::Result<Option<Pending>> {
    let (transition, identity) = {
        let mut file = match state.read(JOURNAL) {
            Ok(file) => file,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(cause) => return Err(cause),
        };
        protection.verify_file(&file)?;
        let transition: Transition =
            serde_json::from_slice(&files::read_bounded(&mut file, LIMIT)?)
                .map_err(io::Error::other)?;
        validate(&transition)?;
        if transition.installation_directory != root.identity()? {
            return Err(invalid("Launcher transition belongs to another installation"));
        }
        (transition, files::file_identity(&file)?)
    };
    let current = record::read(state, protection, context)?
        .ok_or_else(|| invalid("Launcher transition has lost established ownership"))?;
    if current != transition.previous && current != transition.next {
        return Err(invalid("Launcher transition conflicts with established ownership"));
    }
    let next_present = match root.read(ENTRY) {
        Ok(mut file) => {
            protection.verify_file(&file)?;
            let bytes = files::length(&file, context)?;
            let digest = payload::hash(&mut file, context)?;
            if bytes == transition.next.bytes && digest == transition.next.sha256 {
                true
            } else if current == transition.previous
                && bytes == transition.previous.bytes
                && digest == transition.previous.sha256
            {
                false
            } else {
                return Err(invalid(
                    "Launcher transition found unknown executable bytes; file preserved",
                ));
            }
        }
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => false,
        Err(cause) => return Err(cause),
    };
    Ok(Some(Pending { transition, identity, current, next_present }))
}

/// A transition names two different launchers and the next one reads the launch pointer.
fn validate(transition: &Transition) -> io::Result<()> {
    if transition.schema_version != 1 || transition.previous == transition.next {
        return Err(invalid("Unsupported or empty launcher transition"));
    }
    require_replacement(&transition.previous, &transition.next)
}

/// The next launcher must be able to serve as the thin stable entry; both records must be valid.
fn require_replacement(previous: &record::Record, next: &record::Record) -> io::Result<()> {
    previous.validate()?;
    next.validate()?;
    LauncherContract::decode(next.contract.as_bytes())
        .map_err(io::Error::other)?
        .require_stable_entry()
        .map_err(io::Error::other)
}
