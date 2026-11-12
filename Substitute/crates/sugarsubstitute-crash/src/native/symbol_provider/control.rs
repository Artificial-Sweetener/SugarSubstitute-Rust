// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe post-call controls even when native work failed, preserving first causes.
use super::cause::ProviderCause;
use sugarsubstitute_contracts::execution::AdmissionError;
pub(super) fn observed<T>(
    result: Result<T, ProviderCause>,
    post: Result<(), AdmissionError>,
    secondary: &mut Option<ProviderCause>,
) -> Result<T, ProviderCause> {
    match (result, post) {
        (Err(first), Err(post)) => {
            if secondary.is_none() {
                *secondary = Some(ProviderCause::Control(post));
            }
            Err(first)
        }
        (Err(first), Ok(())) => Err(first),
        (Ok(_), Err(post)) => Err(ProviderCause::Control(post)),
        (Ok(value), Ok(())) => Ok(value),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    #[test]
    fn failed_native_result_preserves_source_and_independent_post_control() {
        let native = windows::core::Error::from(windows::core::HRESULT(0x80004005u32 as i32));
        let mut secondary = None;
        let result = observed::<()>(
            Err(ProviderCause::Native(native)),
            Err(AdmissionError::ShuttingDown),
            &mut secondary,
        );
        assert!(
            result
                .as_ref()
                .err()
                .and_then(Error::source)
                .and_then(|cause| cause.downcast_ref::<windows::core::Error>())
                .is_some()
        );
        assert!(
            secondary
                .as_ref()
                .and_then(Error::source)
                .and_then(|cause| cause.downcast_ref::<AdmissionError>())
                .is_some()
        );
    }
    #[test]
    fn successful_native_result_requires_successful_post_control() {
        let mut secondary = None;
        assert!(matches!(
            observed(Ok(17), Err(AdmissionError::ShuttingDown), &mut secondary),
            Err(ProviderCause::Control(AdmissionError::ShuttingDown))
        ));
        assert!(secondary.is_none());
        assert_eq!(observed(Ok(17), Ok(()), &mut secondary).ok(), Some(17));
    }
}
