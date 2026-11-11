// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fixed raw bytes exercise source-write splits independently of native read boundaries.
pub(super) const OUT_PARTS: &[&[u8]] = &[
    b"start ", b"\xe2", b"\x82", b"\xac\n", b"\x1b[", b"31mred", b"\x1b", b"[0m\n", b"old", b"\r",
    b"new\n",
];
pub(super) const ERR_PARTS: &[&[u8]] = &[
    b"error ",
    b"\xf0\x9f",
    b"\x8c",
    b"\xb8\n",
    b"\x1b[3",
    b"2mgreen\x1b[0m\n",
    b"one\r",
    b"two\n",
];
pub(super) const OUT_EXPECTED: &[u8] = b"start \xe2\x82\xac\n\x1b[31mred\x1b[0m\nold\rnew\n";
pub(super) const ERR_EXPECTED: &[u8] = b"error \xf0\x9f\x8c\xb8\n\x1b[32mgreen\x1b[0m\none\rtwo\n";
pub(super) const FLOOD_CHUNKS: u64 = 80;
pub(super) const FLOOD_BYTES: u64 = FLOOD_CHUNKS * 32768;
pub(super) const LEAF_OUT: &[u8] = b"descendant stdout writer retained\n";
pub(super) const LEAF_ERR: &[u8] = b"descendant stderr writer retained\n";
