// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare incremental maximal-subpart replacement with the standard UTF-8 oracle.
use super::Decoder;
#[test]
fn all_single_bytes_and_boundary_prefixes_match_standard_replacement() {
    for prefix in [
        b"".as_slice(),
        &[0xc2],
        &[0xe0],
        &[0xed],
        &[0xf0],
        &[0xf4],
        &[0xe2, 0x82],
        &[0xf0, 0x9f, 0x98],
    ] {
        for byte in 0..=255 {
            let mut input = prefix.to_vec();
            input.push(byte);
            input.extend_from_slice(b"END");
            let mut decoder = Decoder::default();
            let mut actual = String::new();
            for byte in &input {
                let result = decoder.push(*byte);
                actual.extend(result.chars[..result.length].iter());
            }
            if let Some(tail) = decoder.finish() {
                actual.push(tail);
            }
            assert_eq!(actual, String::from_utf8_lossy(&input));
        }
    }
}
