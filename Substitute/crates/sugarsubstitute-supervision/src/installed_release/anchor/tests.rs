// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
use super::embedded;

const HEADER: [u8; 27] = [
    0x52, 0x53, 0x41, 0x31, 0x00, 0x0c, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x80, 0x01, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01,
];

const fn blob(header: [u8; 27]) -> [u8; 411] {
    let mut bytes = [0xa5; 411];
    let mut index = 0;
    while index < header.len() {
        bytes[index] = header[index];
        index += 1;
    }
    bytes
}

static RELEASE: [u8; 411] = blob(HEADER);
static WRONG_EXPONENT: [u8; 411] = {
    let mut header = HEADER;
    header[26] = 0x03;
    blob(header)
};
static WRONG_SIZE: [u8; 411] = {
    let mut header = HEADER;
    header[5] = 0x08;
    blob(header)
};

#[test]
fn release_anchor_names_the_sha256_of_its_exact_public_blob() -> Result<(), &'static str> {
    let anchor = embedded(&RELEASE).ok_or("a well-formed release anchor is trusted")?;
    let expected = "e0954c0115e514246624a4be98937012f938120643c8aedaf4028e92ebfe1584";
    let key_id: String = anchor.key_id.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(key_id, expected);
    assert!(std::ptr::eq(anchor.public_blob, &RELEASE));
    Ok(())
}

#[test]
fn absent_or_malformed_release_anchor_trusts_no_identity() {
    assert!(embedded(&[]).is_none());
    assert!(embedded(&RELEASE[..410]).is_none());
    assert!(embedded(&WRONG_EXPONENT).is_none());
    assert!(embedded(&WRONG_SIZE).is_none());
}

#[cfg(not(feature = "local-k1-symbol-trust"))]
#[test]
fn build_without_a_release_anchor_trusts_no_release_identity() {
    if option_env!("SUGARSUBSTITUTE_RELEASE_ANCHOR").is_none() {
        assert!(super::selected().is_none());
    }
}
