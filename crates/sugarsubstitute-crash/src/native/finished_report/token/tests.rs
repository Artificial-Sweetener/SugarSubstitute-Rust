// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check the actual compiled complete token destination after split guard admission.
use super::*;
#[test]
fn complete_token_and_all_synchronous_cells_fit_same_rust_destination() {
    let total =
        size_of::<Fields>() + super::super::state::RUST_TEMPORARY_BYTES + 2 * size_of::<usize>();
    assert!(total < RUST_DESTINATION_BYTES);
    assert!(align_of::<Fields>() <= 16);
    eprintln!(
        "finished completeFields={} temporaries={} cpp={} rust={} total={} alignment={}",
        size_of::<Fields>(),
        super::super::state::RUST_TEMPORARY_BYTES,
        size_of::<ProtectedFinishedOwnerAllocation>(),
        size_of::<ProtectedFinishedRustRetention>(),
        total,
        align_of::<Fields>()
    );
}

#[path = "tests/abi_boundary.rs"]
mod abi_boundary;
#[path = "tests/runtime.rs"]
mod runtime;
#[path = "tests/support.rs"]
mod support;

#[path = "tests/collection.rs"]
mod collection;
