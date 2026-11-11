// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound operational history separately from authoritative incident records.
//!
//! Calls run on finite admitted execution workers. History lives below the caller's
//! installation root. Shared durable storage owns account verification, protection,
//! native identity pins, staging and publication.
//! Only recognized schema-one pages rotate. A page stage abandoned by an interrupted
//! writer is retired under writer custody; other unknown entries and incident stores
//! remain intact. No native handle or storage lease escapes a call.
#[cfg(any(test, feature = "boundary-fixtures"))]
use crate::durable_storage::protected::directories::Mode;
use crate::durable_storage::{
    check,
    files::{self, Directory},
    invalid, protection, retirement,
    streamed_record::{StreamRead, StreamStage},
};
use serde::{Deserialize, Serialize};
use std::{io, path::Path};
use sugarsubstitute_contracts::diagnostics::{
    DIAGNOSTIC_BATCH_EVENTS, DIAGNOSTIC_EVENT_BYTES, DIAGNOSTIC_HISTORY_PAGES,
    DIAGNOSTIC_PAGE_BYTES, OPERATIONAL_DIAGNOSTIC_SCHEMA,
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
mod contention;
mod export;
#[cfg(test)]
use export::export_at;
pub use export::{OPERATIONAL_EXPORT_BYTES, export};
mod history_import;
#[cfg(test)]
pub(crate) use history_import::seed_earlier_history;
pub(crate) use history_import::{HistorySource, import_original, with_closed_source};
#[cfg(feature = "boundary-fixtures")]
#[doc(hidden)]
pub mod fixture;

const MARKER: &[u8] = b"SugarSubstitute.operational-diagnostics.v1\n";
const DIRECTORY: &str = "operational-v1";
const PAGE_BYTES: usize = DIAGNOSTIC_PAGE_BYTES;
const PAGES: usize = DIAGNOSTIC_HISTORY_PAGES;
/// Maximum UTF-8 bytes in one routed event, including its producer and correlation fields.
pub const OPERATIONAL_EVENT_BYTES: usize = DIAGNOSTIC_EVENT_BYTES;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Page {
    schema: u32,
    sequence: u64,
    omitted: u64,
    events: Vec<String>,
}
/// Bounded UTF-8 export retaining its original admitted memory until the last byte is dropped.
pub struct OperationalDiagnosticExport {
    bytes: Vec<u8>,
    _memory: MemoryClaim,
}
impl OperationalDiagnosticExport {
    /// Borrow the chronological redacted copy without gaining mutation or filesystem authority.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}
/// Recognize only this owner's failed native writer admission, preserving its original cause.
/// Other sharing, permission, publication or protection failures remain terminal.
pub fn writer_busy(cause: &io::Error) -> bool {
    contention::is_busy(cause)
}
/// Publish up to three pre-redacted host events below `<base>\runtime\operational-v1`.
///
/// `base` is the installation root (or setup workspace) named by the configuration layout
/// owner; history never reaches account profile folders. A missing base is created.
///
/// # Errors
/// Preserves original I/O and native retirement causes. Rejects overload,
/// unknown schemas/content, foreign protection, writer contention and controls.
pub fn append(
    base: &Path,
    events: &[String],
    omitted: u64,
    context: &JobContext,
) -> io::Result<()> {
    protection::with(context, |protection| {
        let path = protection.runtime_storage(base, DIRECTORY, true, context)?;
        append_at(&path, events, omitted, protection, context)
    })
}
/// Validate admitted events before protected exclusive publication in the selected namespace.
fn append_at(
    path: &Path,
    events: &[String],
    omitted: u64,
    protection: &mut protection::Protection,
    context: &JobContext,
) -> io::Result<()> {
    if events.len() > DIAGNOSTIC_BATCH_EVENTS || events.iter().any(|event| !valid_event(event)) {
        return Err(invalid("Operational event batch exceeds admission"));
    }
    let _memory = context.memory().claim(2 * 1024 * 1024).map_err(io::Error::other)?;
    let root = Directory::open(path, false)?.resolved()?;
    let _writer = contention::acquire(protection, &root.path().join("writer"), context)?;
    let entries = inventory(&root, protection, context)?;
    match read(&root, "owner", context) {
        Ok(bytes) if bytes == MARKER => {}
        Ok(_) => return Err(invalid("Operational diagnostic owner changed")),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound && entries == 1 => {
            write(&root, "owner", MARKER, context)?
        }
        Err(cause) => return Err(cause),
    }
    let pages = pages(&root, context)?;
    let sequence = pages
        .iter()
        .map(|(_, page)| page.sequence)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| invalid("Operational sequence exhausted"))?;
    let slot = (0..PAGES)
        .find(|slot| !pages.iter().any(|(index, _)| index == slot))
        .or_else(|| pages.iter().min_by_key(|(_, page)| page.sequence).map(|(slot, _)| *slot))
        .ok_or_else(|| invalid("Operational rotation has no slot"))?;
    let bytes = serde_json::to_vec(&Page {
        schema: OPERATIONAL_DIAGNOSTIC_SCHEMA,
        sequence,
        omitted,
        events: events.to_vec(),
    })
    .map_err(io::Error::other)?;
    write(&root, &format!("page-{slot}.json"), &bytes, context).map_err(contention::page_held)
}

/// Refuse unknown siblings before publication and return the recognized entry count.
///
/// The caller holds the exclusive writer rendezvous, and every page stage is created under it,
/// so a stage visible now belongs to a writer that was terminated, cancelled or expired before
/// publishing. Such a stage never became a page; it is verified and retired so one interrupted
/// write cannot block every later launch. Anything else unrecognized stays intact and fails.
fn inventory(
    root: &Directory,
    protection: &protection::Protection,
    context: &JobContext,
) -> io::Result<usize> {
    let mut inventory = crate::RecordDirectory::open(root.path(), false, false, context)?;
    let mut entries = inventory.page(true, context)?;
    // Interrupted stages can push the directory past one bounded page; observe every entry.
    loop {
        let next = inventory.page(false, context)?;
        if next.is_empty() {
            break;
        }
        entries.extend(next);
    }
    let mut recognized = 0;
    let mut interrupted = Vec::new();
    for entry in &entries {
        if !entry.directory && interrupted_page_stage(&entry.name) {
            interrupted.push(entry.name.as_str());
        } else if !entry.directory
            && (entry.name == "owner"
                || entry.name == "writer"
                || (0..PAGES).any(|slot| entry.name == format!("page-{slot}.json")))
        {
            recognized += 1;
        } else {
            return Err(invalid("Unrecognized operational diagnostic storage is preserved"));
        }
    }
    if recognized > PAGES + 2 {
        return Err(invalid("Unrecognized operational diagnostic storage is preserved"));
    }
    for name in interrupted {
        retire_interrupted_stage(root, name, protection, context)?;
    }
    Ok(recognized)
}
/// Match only the exact sibling names `StreamStage` allocates for page publication.
fn interrupted_page_stage(name: &str) -> bool {
    name.strip_prefix("record-stream-").is_some_and(|identity| {
        let guid = identity.len() == 36
            && identity.char_indices().all(|(index, ch)| {
                if matches!(index, 8 | 13 | 18 | 23) {
                    ch == '-'
                } else {
                    ch.is_ascii_digit() || ('A'..='F').contains(&ch)
                }
            });
        let compact = identity.len() == 32
            && identity.chars().all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch));
        guid || compact
    })
}
/// Retire one never-published stage only while it is a protected, single-link regular file
/// within page admission, deleting exactly the inspected native identity.
fn retire_interrupted_stage(
    root: &Directory,
    name: &str,
    protection: &protection::Protection,
    context: &JobContext,
) -> io::Result<()> {
    // The bounded stream reader rejects links, foreign protection and stages beyond page admission.
    drop(StreamRead::open(root, name, PAGE_BYTES as u64, context)?);
    let parent = root.retained()?;
    let file = parent.read(name)?;
    files::single_link(&file)?;
    protection.verify_file(&file)?;
    let identity = files::file_identity(&file)?;
    drop(file);
    retirement::file(&parent, name, identity)
}
/// Reject future schemas, duplicate sequences and invalid events without changing original bytes.
fn pages(root: &Directory, context: &JobContext) -> io::Result<Vec<(usize, Page)>> {
    let mut pages = Vec::with_capacity(PAGES);
    for slot in 0..PAGES {
        let bytes = match read(root, &format!("page-{slot}.json"), context) {
            Ok(bytes) => bytes,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => continue,
            Err(cause) => return Err(cause),
        };
        let page: Page = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        if page.schema != OPERATIONAL_DIAGNOSTIC_SCHEMA
            || page.sequence == 0
            || page.events.len() > DIAGNOSTIC_BATCH_EVENTS
            || page.events.iter().any(|event| !valid_event(event))
            || pages.iter().any(|(_, old): &(usize, Page)| old.sequence == page.sequence)
        {
            return Err(invalid("Operational diagnostic page is incompatible"));
        }
        pages.push((slot, page));
    }
    Ok(pages)
}
/// Keep JSON expansion bounded and allow only the newline framing produced by the route.
fn valid_event(event: &str) -> bool {
    event.len() <= OPERATIONAL_EVENT_BYTES && !event.chars().any(|ch| ch.is_control() && ch != '\n')
}
/// Stream one page under the original execution context and strict page admission.
fn read(root: &Directory, leaf: &str, context: &JobContext) -> io::Result<Vec<u8>> {
    let mut source = StreamRead::open(root, leaf, PAGE_BYTES as u64, context)?;
    let mut bytes = Vec::new();
    while !source.done() {
        bytes.extend_from_slice(&source.next(context)?);
    }
    Ok(bytes)
}
/// Publish through the shared retained staging mechanism after validating exact byte admission.
fn write(root: &Directory, leaf: &str, bytes: &[u8], context: &JobContext) -> io::Result<()> {
    if bytes.len() > PAGE_BYTES {
        return Err(invalid("Operational page exceeds admission"));
    }
    let mut staged = StreamStage::plan(root, leaf, bytes.len() as u64, context)?.create(context)?;
    for chunk in bytes.chunks(8192) {
        staged.append(chunk, context)?;
    }
    staged.commit(context)
}

#[cfg(test)]
mod tests;
