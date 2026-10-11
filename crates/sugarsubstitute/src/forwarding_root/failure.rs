// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer typed terminals without displaying private worker panic text.
use super::*;
use std::fmt;
struct PanicFailure {
    original: std::io::Error,
}
impl fmt::Debug for PanicFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PanicFailure").finish_non_exhaustive()
    }
}
impl fmt::Display for PanicFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Forwarding fixture worker panicked")
    }
}
impl Error for PanicFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.original)
    }
}
pub(super) fn terminal<T>(
    outcome: JobOutcome<T, sugarsubstitute_supervision::RegistryError>,
) -> Result<T> {
    match outcome {
        JobOutcome::Success(value) => Ok(value),
        JobOutcome::Failure(cause) => Err(Box::new(cause)),
        JobOutcome::Panicked(cause) => Err(Box::new(PanicFailure {
            original: std::io::Error::other(cause.as_str().to_owned()),
        })),
        _ => Err("Root finite job stopped without successful actual closure".into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forwarding_panic_top_level_diagnostics_redact_private_payload() -> Result<()> {
        let error = terminal::<()>(JobOutcome::Panicked(PanicCause::new(
            "private-fixture-payload".to_owned(),
        )))
        .err()
        .ok_or("Missing panic failure")?;
        assert!(!format!("{error} {error:?}").contains("private-fixture-payload"));
        assert_eq!(
            error.source().ok_or("Missing original panic")?.to_string(),
            "private-fixture-payload"
        );
        Ok(())
    }
}
