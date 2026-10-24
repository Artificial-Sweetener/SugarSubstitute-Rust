// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish a stable shell entry only after its complete desktop generation is ready.
use super::invalid;
use crate::durable_storage::{
    files::{self, Directory},
    staging,
};
use std::{io, path::Path};
use sugarsubstitute_contracts::execution::JobContext;
#[allow(unsafe_code)]
pub(super) mod native;
pub(super) mod stable;

const NAME: &str = "SugarSubstitute.lnk";

/// Keep the original link bytes beside their immutable completed generation.
pub(super) fn stage(generation: &Path) -> io::Result<()> {
    let directory = Directory::open(generation, false)?;
    let _executable = directory.read("sugarsubstitute.exe")?;
    let destination = generation.join(NAME);
    match directory.read(NAME) {
        Ok(_original) => {
            return if native::target(&destination)? == generation.join("sugarsubstitute.exe") {
                Ok(())
            } else {
                Err(invalid(
                    "The generation's existing shell entry was modified; it has been preserved",
                ))
            };
        }
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
        Err(cause) => return Err(cause),
    }
    let bytes = link_bytes(&directory, &generation.join("sugarsubstitute.exe"))?;
    files::write_immutable(&destination, &bytes, files::create_new)
}

/// Generate complete shell bytes on the storage worker; the caller owns final publication.
fn link_bytes(directory: &Directory, executable: &Path) -> io::Result<Vec<u8>> {
    let staged = staging::unique_path(directory, ".setup-link-stage")?;
    native::create(
        &staged,
        executable,
        executable.parent().ok_or_else(|| invalid("Shell target parent absent"))?,
    )?;
    let leaf = staged
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid("Staged shell entry name is invalid"))?;
    files::read_bounded(&mut directory.read(leaf)?, 65536)
}

/// Unknown or edited links are preserved; only exact retained setup output is replaceable.
pub(super) fn selected(root: &Directory, context: &JobContext) -> io::Result<Option<Directory>> {
    super::check(context)?;
    let mut existing = match root.read(NAME) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(cause) => return Err(cause),
    };
    let bytes = files::read_bounded(&mut existing, 65536)?;
    let target = native::target(&root.path().join(NAME))?;
    files::lexical(&target)?;
    let generation = target.parent().ok_or_else(|| invalid("Shell entry target is absent"))?;
    let directory = Directory::open(generation, false)?;
    if target.file_name().and_then(|name| name.to_str()) != Some("sugarsubstitute.exe")
        || crate::durable_storage::files::canonical_path(
            generation.parent().ok_or_else(|| invalid("Shell entry generation is absent"))?,
            context,
        )? != crate::durable_storage::files::canonical_path(
            root.path().join("releases"),
            context,
        )?
    {
        return Err(invalid("The existing shell entry belongs to another installation"));
    }
    let original = files::read_bounded(&mut directory.read(NAME)?, 65536)?;
    if bytes != original {
        return Err(invalid("The existing shell entry was modified; it has been preserved"));
    }
    completed_record(&directory)?;
    Ok(Some(directory))
}

/// Earlier installer publication is allowed only before protected selection is enabled.
/// Hold the first-enablement lock through publication, alongside the caller's setup exclusion.
pub(super) fn publish(root: &Directory, generation: &Path, context: &JobContext) -> io::Result<()> {
    super::selection::publication::legacy(root, context, || replace(root, generation, context))
}

/// Restore a verified accepted application's shell entry under its run, setup and writer guards.
/// Validate the old entry before staging anything; edited or unknown links remain untouched.
pub(super) fn repair(root: &Directory, generation: &Path, context: &JobContext) -> io::Result<()> {
    selected(root, context)?;
    let directory = Directory::open(generation, false)?;
    ready(&directory)?;
    stage(generation)?;
    super::check(context)?;
    replace(root, generation, context)
}

/// Atomic same-volume publication leaves either the previous complete link or the new one.
/// Callers retain publication authority and installation mutation exclusion until it returns.
fn replace(root: &Directory, generation: &Path, context: &JobContext) -> io::Result<()> {
    super::check(context)?;
    let directory = Directory::open(generation, false)?;
    ready(&directory)?;
    let bytes = files::read_bounded(&mut directory.read(NAME)?, 65536)?;
    let source = staging::unique_path(root, ".setup-entry-stage")?;
    files::write_new(&source, &bytes)?;
    let replace = selected(root, context)?.is_some();
    if replace {
        staging::replace_record(&source, &root.path().join(NAME))
    } else {
        staging::publish(&source, &root.path().join(NAME))
    }
}

fn ready(directory: &Directory) -> io::Result<()> {
    completed_record(directory)?;
    let _executable = directory.read("sugarsubstitute.exe")?;
    Ok(())
}

fn completed_record(directory: &Directory) -> io::Result<()> {
    let record: serde_json::Value = serde_json::from_slice(&files::read_bounded(
        &mut directory.read("installation-ready.json")?,
        1024 * 1024,
    )?)
    .map_err(io::Error::other)?;
    if record["schema_version"] != 1
        || record["bundle_sha256"]
            .as_str()
            .is_none_or(|value| value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(invalid("The shell entry target is not a completed installation"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup_storage::test_worker::{self as worker, TestResult};

    fn generation(root: &Directory, name: &str) -> io::Result<std::path::PathBuf> {
        let path = root.path().join("releases").join(name);
        let _directory = Directory::open(&path, true)?;
        files::write_new(&path.join("sugarsubstitute.exe"), b"unexecuted fixture")?;
        stage(&path)?;
        Ok(path)
    }
    fn complete(path: &Path) -> io::Result<()> {
        files::write_new(
            &path.join("installation-ready.json"),
            &serde_json::to_vec(&serde_json::json!({
                "schema_version":1,"bundle_sha256":"a".repeat(64),
            }))?,
        )
    }

    #[test]
    fn stable_shell_entry_updates_only_between_completed_generations() -> TestResult {
        worker::run(|context, _| {
            let temporary = tempfile::tempdir()?;
            let root = Directory::open(temporary.path(), false)?;
            let first = generation(&root, "first install")?;
            assert!(publish(&root, &first, context).is_err());
            assert!(!root.path().join(NAME).exists());
            complete(&first)?;
            publish(&root, &first, context)?;
            assert_eq!(native::target(&root.path().join(NAME))?, first.join("sugarsubstitute.exe"));
            let old = std::fs::read(root.path().join(NAME))?;
            let second = generation(&root, "第二 糖")?;
            assert!(publish(&root, &second, context).is_err());
            assert_eq!(std::fs::read(root.path().join(NAME))?, old);
            complete(&second)?;
            publish(&root, &second, context)?;
            assert_eq!(
                native::target(&root.path().join(NAME))?,
                second.join("sugarsubstitute.exe")
            );
            assert_eq!(std::fs::read(first.join(NAME))?, old);
            assert!(selected(&root, context)?.is_some());
            Ok(())
        })
    }

    #[test]
    fn modified_and_unknown_shell_entries_are_never_overwritten() -> TestResult {
        worker::run(|context, _| {
            use std::io::Write;
            let temporary = tempfile::tempdir()?;
            let root = Directory::open(temporary.path(), false)?;
            let first = generation(&root, "first")?;
            complete(&first)?;
            publish(&root, &first, context)?;
            std::fs::OpenOptions::new()
                .append(true)
                .open(root.path().join(NAME))?
                .write_all(b"edit")?;
            let edited = std::fs::read(root.path().join(NAME))?;
            let second = generation(&root, "second")?;
            complete(&second)?;
            assert!(publish(&root, &second, context).is_err());
            assert_eq!(std::fs::read(root.path().join(NAME))?, edited);
            std::fs::write(root.path().join(NAME), b"user file")?;
            assert!(publish(&root, &second, context).is_err());
            assert_eq!(std::fs::read(root.path().join(NAME))?, b"user file");
            Ok(())
        })
    }
}
