// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the non-sensitive links of a fatal cause chain readable in routed diagnostics.
use std::{error::Error, fmt};

/// Most links one chain shows; deeper causes are summarized by a count.
const LINKS: usize = 16;
/// Most bytes one chain emits, well inside the routed field limit, so it is never dropped whole.
const BYTES: usize = 1024;
const SEPARATOR: &str = "; caused by: ";
const REDACTED: &str = "[redacted]";

/// Display an error and its sources for a `cause_chain` field.
///
/// Each link is judged on its own text: a link that names a path, URL, credential or other
/// sensitive text is replaced by `[redacted]` while the remaining links stay readable, so an
/// operator learns which owner failed and why without receiving private detail. A startup
/// configuration error ends the chain, because its sources quote configuration content.
pub(crate) struct CauseChain<'a>(pub(crate) &'a (dyn Error + 'static));

impl fmt::Display for CauseChain<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut written = 0;
        let mut current = Some(self.0);
        for depth in 0..LINKS {
            let Some(cause) = current else { return Ok(()) };
            let text = cause.to_string();
            let link =
                if super::fields::sensitive("cause_chain", &text) { REDACTED } else { &text };
            let separator = if depth == 0 { "" } else { SEPARATOR };
            let available = BYTES.saturating_sub(written + separator.len());
            if link.len() > available {
                let end = (0..=available.saturating_sub(3))
                    .rev()
                    .find(|index| link.is_char_boundary(*index))
                    .unwrap_or(0);
                return write!(f, "{separator}{}...", &link[..end]);
            }
            f.write_str(separator)?;
            f.write_str(link)?;
            written += separator.len() + link.len();
            if cause
                .downcast_ref::<sugarsubstitute_configuration::startup::StartupConfigurationError>()
                .is_some()
            {
                return Ok(());
            }
            current = cause.source();
        }
        let mut deeper = 0_usize;
        while let Some(cause) = current.filter(|_| deeper < usize::from(u16::MAX)) {
            deeper += 1;
            current = cause.source();
        }
        if deeper == 0 { Ok(()) } else { write!(f, "{SEPARATOR}{deeper} deeper causes") }
    }
}

#[cfg(test)]
#[path = "cause_chain_tests.rs"]
mod tests;
