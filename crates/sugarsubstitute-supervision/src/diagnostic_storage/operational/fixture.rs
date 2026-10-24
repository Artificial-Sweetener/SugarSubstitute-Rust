// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Isolate actual native storage qualification without changing the account's running application.
use super::*;
/// Exercise the exact native page owner below one explicitly selected private fixture directory.
/// # Errors
/// Preserves actual protection, publication, admission and retirement failures.
pub fn append(
    path: &Path,
    events: &[String],
    omitted: u64,
    context: &JobContext,
) -> io::Result<()> {
    protection::with(context, |protection| {
        protection.directory(path, Mode::OpenOrCreate, context)?;
        append_at(path, events, omitted, protection, context)
    })
}
