// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read the version sections semantic-release writes into a component's `CHANGELOG.md`.
//!
//! A minor release is headed `# [x.y.z](compare) (date)` and a patch `## [x.y.z](compare) (date)`;
//! a release without a compare link is headed `## x.y.z (date)`. A section runs from its version
//! heading to the next version heading, whatever its level, and keeps its heading, so the notes
//! show the release's date and comparison link exactly as published.

use std::cmp::Ordering;
use sugarsubstitute_contracts::host_release_notes::{
    MAX_NOTES_UNITS, MAX_RELEASES, ReleaseNotesRelease, ReleaseNotesVersion,
};

/// Most UTF-8 bytes of notes one answer carries, leaving room for the envelope and escaping
/// within the release notes message bound.
pub const MAX_NOTES_BYTES: usize = 768 * 1024;

/// The sections of a selected range, newest first, and how many older ones were left out.
#[derive(Debug, Eq, PartialEq)]
pub struct Selection {
    /// One to [`MAX_RELEASES`] versions.
    pub releases: Vec<ReleaseNotesRelease>,
    /// Older versions in the range left out to stay within the bounds.
    pub omitted: u64,
}

/// A plain release version, ordered as releases are: numerically, and a release after its own
/// prereleases.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Version {
    core: [u64; 3],
    prerelease: Option<String>,
}

impl Version {
    fn parse(text: &str) -> Option<Self> {
        let (core, prerelease) = match text.split_once('-') {
            Some((core, prerelease)) if !prerelease.is_empty() => (core, Some(prerelease)),
            Some(_) => return None,
            None => (text, None),
        };
        let mut parts = core.split('.');
        let mut number = || parts.next()?.parse::<u64>().ok();
        let core = [number()?, number()?, number()?];
        if parts.next().is_some() {
            return None;
        }
        Some(Self { core, prerelease: prerelease.map(str::to_owned) })
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core.cmp(&other.core).then_with(|| match (&self.prerelease, &other.prerelease) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(left), Some(right)) => left.cmp(right),
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// The version a line heads, when it is a version heading.
fn heading(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("## ").or_else(|| line.strip_prefix("# "))?;
    let version = match rest.strip_prefix('[') {
        Some(linked) => linked.split_once("](")?.0,
        None => rest.split(|character: char| character.is_whitespace()).next()?,
    };
    Version::parse(version).map(|_| version)
}

/// Every version section in `changelog`, in the order the file lists them.
fn sections(changelog: &str) -> Vec<(&str, &str)> {
    let mut starts = Vec::new();
    let mut offset = 0;
    for line in changelog.split_inclusive('\n') {
        if let Some(version) = heading(line.trim_end_matches(['\r', '\n'])) {
            starts.push((offset, version));
        }
        offset += line.len();
    }
    starts
        .iter()
        .enumerate()
        .map(|(index, (start, version))| {
            let end = starts.get(index + 1).map_or(changelog.len(), |next| next.0);
            (*version, changelog[*start..end].trim_end())
        })
        .collect()
}

/// Cut `text` at the last line break that keeps it within `units` UTF-16 code units and
/// `bytes` UTF-8 bytes; text without such a break is cut at the last fitting character.
fn bounded(text: &str, units: usize, bytes: usize) -> &str {
    let mut counted = 0;
    let mut fits = text.len();
    for (index, character) in text.char_indices() {
        counted += character.len_utf16();
        if counted > units || index + character.len_utf8() > bytes {
            fits = index;
            break;
        }
    }
    if fits == text.len() {
        return text;
    }
    let cut = &text[..fits];
    cut.rfind('\n').map_or(cut, |line| &cut[..line]).trim_end()
}

/// The sections after `since` through `running`, newest first, or `None` when the changelog
/// has no section for `running`. A `since` that is not older than `running`, or is not a
/// release version, selects `running` alone.
pub fn select(changelog: &str, running: &str, since: &str) -> Option<Selection> {
    let current = Version::parse(running)?;
    let since = Version::parse(since).filter(|since| *since < current);
    // A heading the release notes protocol cannot name, such as an uppercase prerelease, is not
    // a section it can show.
    let mut found: Vec<(Version, ReleaseNotesVersion, &str)> = sections(changelog)
        .into_iter()
        .filter_map(|(label, text)| {
            Some((Version::parse(label)?, ReleaseNotesVersion::new(label).ok()?, text))
        })
        .filter(|(version, ..)| match &since {
            Some(since) => version > since && *version <= current,
            None => *version == current,
        })
        .collect();
    if !found.iter().any(|(version, ..)| *version == current) {
        return None;
    }
    found.sort_by(|left, right| right.0.cmp(&left.0));
    found.dedup_by(|later, earlier| later.0 == earlier.0);
    let total = found.len();
    let mut releases = Vec::new();
    let mut spent = 0;
    for (_, version, text) in &found {
        let remaining = MAX_NOTES_BYTES.saturating_sub(spent);
        if releases.len() == MAX_RELEASES || (remaining == 0 && !releases.is_empty()) {
            break;
        }
        let notes = bounded(text, MAX_NOTES_UNITS, remaining.max(1));
        if notes.is_empty() && !releases.is_empty() {
            break;
        }
        spent += notes.len();
        releases.push(ReleaseNotesRelease::new(version.clone(), notes.to_owned()).ok()?);
    }
    let omitted = (total - releases.len()) as u64;
    Some(Selection { releases, omitted })
}
