// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind the source-owned actual Rust cells/temporaries to the closed destination.
use super::*;
#[test]
fn retained_and_synchronous_cells_fit_source_owned_rust_destination() {
    assert!(
        size_of::<State>() + RUST_TEMPORARY_BYTES + 2 * size_of::<usize>() < RUST_DESTINATION_BYTES
    );
    assert!(align_of::<State>() <= 16);
    eprintln!(
        "finished-rust state={} temporary={} sourcelease={} alignment={}",
        size_of::<State>(),
        RUST_TEMPORARY_BYTES,
        size_of::<sugarsubstitute_supervision::diagnostic_storage::ProtectedCaptureReadLease>(),
        align_of::<State>()
    );
}
