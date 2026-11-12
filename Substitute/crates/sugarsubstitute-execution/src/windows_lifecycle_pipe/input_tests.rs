// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject malformed inherited tokens before admission or native adoption.
use super::*;
#[test]
fn inherited_bootstrap_token_is_positive_pointer_sized_and_non_pseudo() {
    assert_eq!(InheritedLifecycleBootstrap::new(17).map(|input| input.raw), Ok(17));
    for value in [0, u64::MAX, (usize::MAX - 15) as u64, usize::MAX as u64] {
        assert_eq!(
            InheritedLifecycleBootstrap::new(value).err(),
            Some(LifecyclePipeError::InvalidMessage)
        );
    }
}

#[test]
fn original_win32_failure_code_is_exact_and_other_causes_remain_distinct() {
    for code in [109, 233, 5, 995] {
        let cause = windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(code));
        let failure =
            LifecyclePipeFailure::native(NativeObservationError::new("pipe_completion", cause));
        assert_eq!(failure.win32_code(), Some(code));
        assert_eq!(failure.native_cause().stage(), "pipe_completion");
        assert!(!failure.requires_process_shutdown());
    }
    let cause = windows::core::Error::from_hresult(windows::core::HRESULT(0x8000_4005u32 as i32));
    assert_eq!(
        LifecyclePipeFailure::native(NativeObservationError::new("other_hresult", cause))
            .win32_code(),
        None
    );
    assert_eq!(lifecycle_pipe::failure("invariant").win32_code(), None);
}

#[test]
fn nonfailure_and_nonwin32_hresult_shapes_never_classify_as_native_disconnect() {
    for value in [0u32, 1, 109, 0x0007_006d, 0x8004_006d, 0x8007_0000] {
        let cause = windows::core::Error::from_hresult(windows::core::HRESULT(value as i32));
        let failure =
            LifecyclePipeFailure::native(NativeObservationError::new("pipe_completion", cause));
        assert_eq!(failure.win32_code(), None);
    }
}
