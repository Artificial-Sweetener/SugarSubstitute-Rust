// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve both bounded recipient failures after delivering one immutable batch.
use std::{error::Error, fmt};

/// Retain the first render failure and the independently observed child failure.
struct DeliveryFailure {
    render: Box<dyn Error>,
    child: Box<dyn Error>,
}
impl fmt::Debug for DeliveryFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DeliveryFailure")
    }
}
impl fmt::Display for DeliveryFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Render and child completion delivery failed")
    }
}
impl Error for DeliveryFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.render.as_ref())
    }
}
impl Drop for DeliveryFailure {
    fn drop(&mut self) {
        tracing::error!(operation="startup_delivery",cause=%self.child,"Additional owned child completion failure");
    }
}
/// Borrow only the known aggregate's first unchanged cause; arbitrary sources are never walked.
pub(super) fn first_cause<'a>(cause: &'a (dyn Error + 'static)) -> &'a (dyn Error + 'static) {
    cause.downcast_ref::<DeliveryFailure>().map_or(cause, |failure| failure.render.as_ref())
}

/// Resolve already-observed recipient outcomes without replacing their actual causes.
/// The caller invokes both recipients before this function, even if one fails.
pub(super) fn resolve<T>(
    render: Result<T, Box<dyn Error>>,
    child: Result<(), Box<dyn Error>>,
) -> Result<T, Box<dyn Error>> {
    match (render, child) {
        (Ok(frame), Ok(())) => Ok(frame),
        (Err(cause), Ok(())) | (Ok(_), Err(cause)) => Err(cause),
        (Err(render), Err(child)) => Err(Box::new(DeliveryFailure { render, child })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn simultaneous_recipient_failures_retain_both_actual_typed_causes()
    -> Result<(), Box<dyn Error>> {
        let failure = resolve::<()>(
            Err(Box::new(std::io::Error::other("render"))),
            Err(Box::new(std::io::Error::other("child"))),
        )
        .err()
        .ok_or("Both failures lost")?;
        let combined = failure.downcast_ref::<DeliveryFailure>().ok_or("Combined custody lost")?;
        assert_eq!(
            failure
                .source()
                .and_then(|e| e.downcast_ref::<std::io::Error>())
                .ok_or("Primary cause lost")?
                .to_string(),
            "render"
        );
        assert_eq!(
            combined
                .child
                .downcast_ref::<std::io::Error>()
                .ok_or("Secondary cause lost")?
                .to_string(),
            "child"
        );
        Ok(())
    }
    #[test]
    fn individual_recipient_failure_keeps_its_original_type_and_success_returns_frame()
    -> Result<(), Box<dyn Error>> {
        for (render, child) in [
            (Err(Box::new(std::io::Error::other("render")) as Box<dyn Error>), Ok(())),
            (Ok(7), Err(Box::new(std::io::Error::other("child")) as Box<dyn Error>)),
        ] {
            assert!(
                resolve(render, child)
                    .err()
                    .ok_or("Failure lost")?
                    .downcast_ref::<std::io::Error>()
                    .is_some()
            );
        }
        assert_eq!(resolve(Ok(7), Ok(()))?, 7);
        Ok(())
    }
}
