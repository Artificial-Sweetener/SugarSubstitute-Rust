// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify the actual linked C ABI rejects malformed input before any borrowed callback.
use super::*;
struct Probe {
    calls: u32,
}
extern "C" fn counted_read(
    context: *mut c_void,
    _offset: u64,
    _output: *mut u8,
    _count: usize,
) -> u32 {
    // Safety: this test supplies its own synchronous nonnull Probe context.
    unsafe {
        (*context.cast::<Probe>()).calls += 1;
    }
    1
}
extern "C" fn counted_check(context: *mut c_void) -> u32 {
    // Safety: the same synchronous test Probe lives through the native call.
    unsafe {
        (*context.cast::<Probe>()).calls += 1;
    }
    3
}
#[test]
fn linked_abi_rejects_wrong_role_version_and_reserved_before_callbacks() {
    for (version, role, reserved) in [(2, 0, 0), (1, 2, 0), (1, 0, 1)] {
        let expected = Expected {
            version,
            role,
            build_length: 1,
            reserved,
            instance: 0,
            generation: 0,
            file_size: 64,
            namespace: [1; 32],
            epoch: [2; 16],
            build: [b'a'; 64],
            report_uuid: [3; 16],
        };
        let mut probe = Probe { calls: 0 };
        let mut result = ResultFacts::default();
        // Safety: exact reprC input/output, valid callbacks and stack context live for the whole call.
        unsafe {
            SugarParseCommittedDump(
                &expected,
                (&mut probe as *mut Probe).cast(),
                counted_read,
                counted_check,
                &mut result,
            );
        }
        assert_eq!(probe.calls, 0);
        assert_eq!(result.version, 1);
        assert_ne!(result.status, 0);
        assert_eq!(result.report_uuid, [0; 16]);
    }
}
