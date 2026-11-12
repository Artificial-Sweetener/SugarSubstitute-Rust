// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Reject real time in test scope, so no test or qualification passes or fails on elapsed seconds.
//!
//! Test code waits for observed outcomes and reads deterministic clocks. In test scope (see
//! [`crate::testing_scope`]) the gate rejects `.elapsed()`, every `wait_timeout*`,
//! `recv_timeout` and `park_timeout*`, `Instant::now`, `SystemTime::now`, `unix_now` and
//! `MonotonicClock::new`, resolving import aliases and macro tokens through the shared capability
//! scanner. The shared frozen anchor and no-progress guard live outside test scope. A file whose
//! real time provably decides no outcome is admitted only by an exact fingerprinted registration
//! with a file-specific reason; stale or unused registrations fail.

use std::{collections::BTreeSet, path::Path};

use proc_macro2::{TokenStream, TokenTree};
use serde::Deserialize;

use crate::{
    capability_source,
    error::{Failure, Result, require},
    policy::Architecture,
    source::{fingerprint, resolve},
    testing_scope::scoped_sources,
};

/// Method names whose result depends on how much real time passed.
const TIMED_METHODS: [&str; 7] = [
    "elapsed",
    "wait_timeout",
    "wait_timeout_while",
    "wait_timeout_ms",
    "recv_timeout",
    "park_timeout",
    "park_timeout_ms",
];

/// Free functions that read the real wall clock. A stored timestamp compared with a fresh read
/// fails whenever the two reads straddle a second boundary.
const WALL_CLOCK_READS: [&str; 1] = ["unix_now"];

/// Associated functions that sample or construct a real clock.
const CLOCK_SOURCES: [[&str; 2]; 3] =
    [["Instant", "now"], ["SystemTime", "now"], ["MonotonicClock", "new"]];

/// One reviewed test-scope file whose real time decides no outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RealTime {
    pub(crate) path: String,
    pub(crate) source_sha256: String,
    pub(crate) reason: String,
}

/// Scan every crate's test scope and bind each finding to an exact current registration.
///
/// # Errors
/// Fails on unregistered real time, a stale fingerprint, a registration without findings, a
/// duplicate, or an empty reason.
pub(crate) fn check(
    root: &Path,
    registered: &[RealTime],
    architecture: &Architecture,
) -> Result<()> {
    let mut paths = BTreeSet::new();
    for registration in registered {
        require(
            paths.insert(registration.path.as_str()),
            format!("testing: {} is registered for real time twice", registration.path),
        )?;
        require(
            registration.reason.split_whitespace().count() >= 12,
            format!("testing: {} needs a file-specific real-time reason", registration.path),
        )?;
        require(
            fingerprint(root, std::slice::from_ref(&registration.path))?
                == registration.source_sha256,
            format!("testing: {} has a stale real-time fingerprint", registration.path),
        )?;
    }
    let mut used = BTreeSet::new();
    let mut unregistered = Vec::new();
    for owner in &architecture.owners {
        for scoped in scoped_sources(&resolve(root, &owner.root)?)? {
            let found = findings(&scoped.source)?;
            if found.is_empty() {
                continue;
            }
            let relative = scoped
                .path
                .strip_prefix(root)
                .unwrap_or(&scoped.path)
                .to_string_lossy()
                .replace('\\', "/");
            if !paths.contains(relative.as_str()) {
                unregistered.push(format!("{relative} ({})", found.join(", ")));
            }
            used.insert(relative);
        }
    }
    require(
        unregistered.is_empty(),
        format!(
            "testing: these files let real time decide tests: {}; await the observed outcome, use \
             a deterministic clock or the shared no-progress guard",
            unregistered.join("; ")
        ),
    )?;
    for path in paths {
        require(
            used.contains(path),
            format!("testing: {path} is registered for real time but its test scope uses none"),
        )?;
    }
    Ok(())
}

/// Return the real-time accesses of one test-scope source.
///
/// # Errors
/// Fails when the source is not valid Rust.
pub(crate) fn findings(source: &str) -> Result<Vec<String>> {
    let mut found: BTreeSet<String> =
        capability_source::findings(source, clock_path, clock_fragments, |_| false)?
            .into_iter()
            .collect();
    let tokens: TokenStream =
        source.parse().map_err(|error| Failure::external("tokenize real-time access", error))?;
    method_calls(tokens, &mut found);
    Ok(found.into_iter().collect())
}

fn clock_path(path: &[String]) -> bool {
    path.windows(2).any(|pair| CLOCK_SOURCES.iter().any(|source| pair == source))
        || path.last().is_some_and(|name| WALL_CLOCK_READS.contains(&name.as_str()))
        || path.len() > 1 && path.last().is_some_and(|name| TIMED_METHODS.contains(&name.as_str()))
}

/// Macro tokens that can only assemble a clock read or a timed wait. `elapsed` alone is an
/// ordinary field and variable name, so only its call syntax counts.
fn clock_fragments(names: &BTreeSet<String>) -> bool {
    CLOCK_SOURCES.iter().any(|[ty, function]| names.contains(*ty) && names.contains(*function))
        || TIMED_METHODS[1..].iter().any(|name| names.contains(*name))
        || WALL_CLOCK_READS.iter().any(|name| names.contains(*name))
}

/// Find `.method(` calls of timed methods anywhere, including inside macro arguments and bodies.
fn method_calls(tokens: TokenStream, found: &mut BTreeSet<String>) {
    let tokens: Vec<_> = tokens.into_iter().collect();
    for (index, token) in tokens.iter().enumerate() {
        if let TokenTree::Group(group) = token {
            method_calls(group.stream(), found);
        }
        if let [TokenTree::Punct(dot), TokenTree::Ident(name), TokenTree::Group(arguments), ..] =
            &tokens[index..]
            && dot.as_char() == '.'
            && arguments.delimiter() == proc_macro2::Delimiter::Parenthesis
            && TIMED_METHODS.contains(&name.to_string().as_str())
        {
            found.insert(format!(".{name}()"));
        }
    }
}

#[cfg(test)]
#[path = "testing_time_tests.rs"]
mod tests;
