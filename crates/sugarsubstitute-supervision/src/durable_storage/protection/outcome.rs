// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep domain failures observable when native account/directory retirement also fails.
use crate::durable_storage::protection::{Protection, with};
use std::io;
use sugarsubstitute_contracts::execution::JobContext;

/// Native retirement never replaces an earlier domain failure.
#[derive(Debug)]
pub(crate) enum DomainFailure<E> {
    /// The operation failed and native custody retired successfully.
    Operation(E),
    /// Native admission or retirement failed, possibly after a domain failure.
    Native { cause: io::Error, operation: Option<E> },
}

impl DomainFailure<io::Error> {
    /// Preserve ordinary I/O classification only after native protection retired successfully.
    pub(crate) fn into_io(self) -> io::Error {
        match self {
            Self::Operation(cause) => cause,
            native => io::Error::other(native),
        }
    }
}
impl<E: std::error::Error> std::fmt::Display for DomainFailure<E> {
    /// Keep the native failure visible when an earlier operation also failed.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Operation(cause) => std::fmt::Display::fmt(cause, formatter),
            Self::Native { cause, .. } => write!(formatter, "Native protection failed: {cause}"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for DomainFailure<E> {
    /// Preserve the original operation as primary while retaining native failure in the variant.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Operation(cause) | Self::Native { operation: Some(cause), .. } => Some(cause),
            Self::Native { cause, operation: None } => Some(cause),
        }
    }
}

/// Run a typed operation while checking all native protection retirement outcomes.
pub(crate) fn with_domain<T, E>(
    context: &JobContext,
    operation: impl FnOnce(&mut Protection) -> Result<T, E>,
) -> Result<T, DomainFailure<E>> {
    let mut result = None;
    let native = with(context, |protection| {
        result = Some(operation(protection));
        Ok(())
    });
    finish(native, result)
}

/// Join independently observed outcomes without losing either cause.
fn finish<T, E>(
    native: io::Result<()>,
    operation: Option<Result<T, E>>,
) -> Result<T, DomainFailure<E>> {
    match (native, operation) {
        (Ok(()), Some(result)) => result.map_err(DomainFailure::Operation),
        (Err(cause), operation) => {
            Err(DomainFailure::Native { cause, operation: operation.and_then(Result::err) })
        }
        (Ok(()), None) => Err(DomainFailure::Native {
            cause: io::Error::other("Protected operation was not observed"),
            operation: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_io_is_repairable_only_after_successful_native_retirement() {
        let absent = || io::Error::new(io::ErrorKind::NotFound, "absent payload");
        let ordinary = DomainFailure::Operation(absent()).into_io();
        assert_eq!(ordinary.kind(), io::ErrorKind::NotFound);
        let native = DomainFailure::Native {
            cause: io::Error::other("retirement failed"),
            operation: Some(absent()),
        }
        .into_io();
        assert_eq!(native.kind(), io::ErrorKind::Other);
        assert!(
            matches!(native.get_ref().and_then(|cause| cause.downcast_ref::<DomainFailure<io::Error>>()),
            Some(DomainFailure::Native { cause, operation: Some(original) })
                if cause.to_string() == "retirement failed" && original.kind() == io::ErrorKind::NotFound)
        );
    }

    #[test]
    fn typed_failure_survives_failed_native_retirement() {
        let result = finish::<(), _>(Err(io::Error::other("retirement")), Some(Err(42)));
        assert!(matches!(result, Err(DomainFailure::Native { cause, operation: Some(42) })
            if cause.to_string() == "retirement"));
        assert!(matches!(
            finish::<(), _>(Ok(()), Some(Err(42))),
            Err(DomainFailure::Operation(42))
        ));
        assert!(matches!(finish::<_, ()>(Ok(()), Some(Ok(42))), Ok(42)));
    }

    #[test]
    fn success_is_not_returned_after_native_failure() {
        assert!(matches!(
            finish::<_, ()>(Err(io::Error::other("retirement")), Some(Ok(42))),
            Err(DomainFailure::Native { operation: None, .. })
        ));
        assert!(matches!(finish::<(), ()>(Ok(()), None), Err(DomainFailure::Native { .. })));
    }
}
