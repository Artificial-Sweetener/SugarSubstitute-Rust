// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Sign and verify the SugarSubstitute update repository from release CI.
//!
//! The repository is a flat set of files: `<n>.root.json` for every root version, `timestamp.json`,
//! `<n>.snapshot.json`, `<n>.targets.json`, channel descriptors and content-addressed packages,
//! exactly the names the application requests. Every operation first authenticates the current
//! published state from a trusted root, then signs the next versions with the lifetimes in
//! [lifetimes], and never rewrites a versioned file. [verify] then runs the application's own
//! verifier over the result, as an installation that already trusted the previous state would.
//!
//! Time is always supplied as Unix UTC seconds; nothing here reads a clock, the environment or
//! the filesystem except [storage], which the command line uses.
pub mod keys;
pub mod lifetimes;
mod repository;
mod sign;
pub mod storage;
mod verify;
pub use repository::{Files, Repository};
pub use sign::{Change, NewTarget, OnlineKeys, Published, initialize};
pub use verify::{Payloads, Report, verify};

/// A repository operation could not be carried out; nothing it would have written is valid.
#[derive(Debug)]
pub enum Error {
    /// Signing, parsing or signature verification failed in the TUF implementation.
    Trust(tuf::Error),
    /// The application's verifier rejected the repository.
    Verifier(sugarsubstitute_update::Error),
    /// A file or key could not be read or written.
    Io(std::io::Error),
    /// The repository or a request is inconsistent; the message says how.
    Invalid(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trust(cause) => write!(f, "TUF metadata error: {cause}"),
            Self::Verifier(cause) => write!(f, "The application's verifier rejected it: {cause:?}"),
            Self::Io(cause) => write!(f, "{cause}"),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Trust(cause) => Some(cause),
            Self::Verifier(cause) => Some(cause),
            Self::Io(cause) => Some(cause),
            Self::Invalid(_) => None,
        }
    }
}
impl From<tuf::Error> for Error {
    fn from(cause: tuf::Error) -> Self {
        Self::Trust(cause)
    }
}
impl From<sugarsubstitute_update::Error> for Error {
    fn from(cause: sugarsubstitute_update::Error) -> Self {
        Self::Verifier(cause)
    }
}
impl From<std::io::Error> for Error {
    fn from(cause: std::io::Error) -> Self {
        Self::Io(cause)
    }
}

/// Sign `change` on top of the published `current` files and verify the result with the
/// application's verifier, as an installation that already accepted `current`, before anything
/// is written. Returns the new files and what installations will accept.
/// # Errors
/// Returns authentication, signing and verification failures; nothing is valid to publish then.
pub fn prepare(
    current: &Files,
    trusted_root: &[u8],
    change: Change<'_>,
    payloads: &dyn Payloads,
    now: i64,
) -> Result<(Published, Report), Error> {
    let published = Repository::authenticate(current, trusted_root)?.sign(change, now)?;
    let mut next = current.clone();
    next.extend(published.files.clone());
    next.extend(published.descriptors.clone());
    let report = verify(trusted_root, Some(current), &next, payloads, now)?;
    Ok((published, report))
}

/// Build an [Error::Invalid] from a message.
pub(crate) fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}
