// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own portable source discovery, safe policy paths, and exact review fingerprints.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{Failure, Result, read, require};

/// Resolve an existing repository-relative path without escaping through links.
pub(crate) fn resolve(root: &Path, relative: &str) -> Result<PathBuf> {
    require(
        !relative.is_empty()
            && !relative.contains(['\\', ':'])
            && relative.split('/').all(|part| !part.is_empty() && part != "." && part != ".."),
        format!("policy path must use relative forward-slash components: {relative}"),
    )?;
    let candidate = root.join(relative);
    let canonical = candidate
        .canonicalize()
        .map_err(|error| Failure::external(format!("resolve {relative}"), error))?;
    let canonical_root =
        root.canonicalize().map_err(|error| Failure::external("resolve repository root", error))?;
    require(
        canonical.starts_with(canonical_root),
        format!("policy path escapes repository: {relative}"),
    )?;
    Ok(candidate)
}

/// Discover Rust files deterministically, rejecting linked source trees.
pub(crate) fn rust_files(directory: &Path) -> Result<Vec<PathBuf>> {
    source_files(directory, &["rs"])
}

/// Discover selected authored languages with identical path and link checks.
pub(crate) fn source_files(directory: &Path, extensions: &[&str]) -> Result<Vec<PathBuf>> {
    source_files_excluding(directory, extensions, &[])
}

/// Exclude only caller-identified installed dependency roots before traversing authored sources.
/// Normal source discovery supplies no exclusions; similarly named nested paths remain governed.
pub(crate) fn source_files_excluding(
    directory: &Path,
    extensions: &[&str],
    external: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| Failure::external(format!("scan {}", directory.display()), error))?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| Failure::external("read source entry", error))?;
        if external.contains(&entry.path()) {
            continue;
        }
        let kind =
            entry.file_type().map_err(|error| Failure::external("read source type", error))?;
        require(
            !kind.is_symlink(),
            format!("linked source path is not governed: {}", entry.path().display()),
        )?;
        if kind.is_dir() {
            require(
                entry.file_name() != "target",
                "keep build outputs outside governed source roots",
            )?;
            paths.extend(source_files_excluding(&entry.path(), extensions, external)?);
        } else if entry.path().extension().and_then(|extension| extension.to_str()).is_some_and(
            |extension| extensions.iter().any(|wanted| extension.eq_ignore_ascii_case(wanted)),
        ) || extensions.contains(&"cmake")
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.eq_ignore_ascii_case("CMakeLists.txt"))
        {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}

/// Fingerprint exact source paths and normalized contents in sorted order.
pub(crate) fn fingerprint(root: &Path, sources: &[String]) -> Result<String> {
    fingerprint_resolved(sources, |relative| resolve(root, relative))
}

/// Fingerprint sources whose recorded paths a caller-owned resolver locates.
///
/// The digest binds each recorded path spelling and its normalized contents, so a caller that
/// resolves some paths outside the workspace still reviews exactly what it records.
pub(crate) fn fingerprint_resolved(
    sources: &[String],
    locate: impl Fn(&str) -> Result<PathBuf>,
) -> Result<String> {
    require(!sources.is_empty(), "reviewed source inventory must not be empty")?;
    let mut sorted = sources.to_vec();
    sorted.sort();
    sorted.dedup();
    require(sorted.len() == sources.len(), "reviewed source inventory contains duplicate paths")?;
    let mut digest = Sha256::new();
    for relative in sorted {
        let source = read(&locate(&relative)?)?;
        digest.update(relative.as_bytes());
        digest.update([0]);
        digest.update(source.as_bytes());
        digest.update([0]);
    }
    Ok(format!("sha256:{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Case variants retain governed language identity on Windows and portable checkouts alike.
    #[test]
    fn source_discovery_includes_case_variant_language_extensions() -> Result<()> {
        let temporary =
            tempfile::tempdir().map_err(|cause| Failure::external("create fixture", cause))?;
        for name in ["owner.RS", "worker.Ts", "native.CPP"] {
            std::fs::write(temporary.path().join(name), "")
                .map_err(|cause| Failure::external("write source fixture", cause))?;
        }
        assert_eq!(source_files(temporary.path(), &["rs", "ts", "cpp"])?.len(), 3);
        Ok(())
    }

    /// Reject path spellings that escape or change meaning across platforms.
    #[test]
    fn policy_paths_reject_escaping_and_platform_specific_forms() -> Result<()> {
        let temporary =
            tempfile::tempdir().map_err(|error| Failure::external("create fixture", error))?;
        for path in ["../outside", "/absolute", "C:/secret", "folder\\file", "a//b", "./a"] {
            assert!(resolve(temporary.path(), path).is_err(), "accepted {path}");
        }
        Ok(())
    }

    /// Keep reviewed identities stable across Windows and Unix newline encodings.
    #[test]
    fn fingerprints_normalize_newlines_and_reject_duplicate_sources() -> Result<()> {
        let temporary =
            tempfile::tempdir().map_err(|error| Failure::external("create fixture", error))?;
        let path = temporary.path().join("file.rs");
        let sources = vec!["file.rs".to_owned()];
        std::fs::write(&path, "a\r\nb\r\n")
            .map_err(|error| Failure::external("write fixture", error))?;
        let first = fingerprint(temporary.path(), &sources)?;
        std::fs::write(&path, "a\nb\n")
            .map_err(|error| Failure::external("write fixture", error))?;
        assert_eq!(first, fingerprint(temporary.path(), &sources)?);
        assert!(fingerprint(temporary.path(), &[sources[0].clone(), sources[0].clone()]).is_err());
        Ok(())
    }
}

#[cfg(test)]
mod build_source_tests {
    use super::*;
    /// Build-language entry files remain visible despite their conventional text extension.
    #[test]
    fn cmake_entry_names_are_discovered_with_build_language_sources() -> Result<()> {
        let root = tempfile::tempdir()
            .map_err(|cause| Failure::external("create source fixture", cause))?;
        std::fs::write(root.path().join("cMaKeLiStS.TxT"), "FILE(WRITE output bytes)")
            .map_err(|cause| Failure::external("write source fixture", cause))?;
        assert_eq!(source_files(root.path(), &["cmake"])?.len(), 1);
        assert!(source_files(root.path(), &["rs"])?.is_empty());
        Ok(())
    }
}
