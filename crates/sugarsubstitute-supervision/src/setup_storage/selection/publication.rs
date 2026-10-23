// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep shell publication subordinate to selection enablement and accepted launch authority.
use super::*;

/// Hold first-enablement exclusion through an earlier installer's complete publication.
/// Selection-enabled installations use the accepted application owner instead. Unknown,
/// incomplete or damaged selection state never restores the earlier publication authority.
pub(in crate::setup_storage) fn legacy<T>(
    installation: &Directory,
    context: &JobContext,
    publish: impl FnOnce() -> io::Result<T>,
) -> io::Result<T> {
    protection::with(context, |protection| {
        let _transition = namespace::transition(installation, protection, context)?;
        for leaf in [DIRECTORY, NAMESPACE_MARKER, RECORD, LOCK] {
            match std::fs::symlink_metadata(installation.path().join(leaf)) {
                Ok(_) => {
                    return Err(super::super::invalid(
                        "This installation requires health-checked update activation; its shortcut has been preserved",
                    ));
                }
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
                Err(cause) => return Err(cause),
            }
        }
        check(context)?;
        publish()
    })
}

#[cfg(test)]
mod tests;
