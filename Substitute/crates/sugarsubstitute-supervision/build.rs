// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Embed the release-manifest public anchor a release build trusts.

use std::{error::Error, io::Read, path::PathBuf};

const VARIABLE: &str = "SUGARSUBSTITUTE_RELEASE_ANCHOR";
const OUTPUT: &str = "release-anchor.bin";
/// CNG `BCRYPT_RSAPUBLIC_BLOB` header for RSA-3072 with a three-byte exponent and no primes.
const HEADER: [u8; 24] = [
    0x52, 0x53, 0x41, 0x31, 0x00, 0x0c, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x80, 0x01, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
const EXPONENT: [u8; 3] = [0x01, 0x00, 0x01];
const BLOB_BYTES: usize = 411;

/// A release build names its anchor file; every other build embeds no release identity.
fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-env-changed={VARIABLE}");
    let output = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("Cargo output directory absent")?)
        .join(OUTPUT);
    let mut bytes = Vec::new();
    if let Some(source) = std::env::var_os(VARIABLE) {
        let path = PathBuf::from(source);
        let text = path.to_str().ok_or("Release anchor path must be UTF-8")?;
        if !path.is_absolute() || text.contains(['\r', '\n']) {
            return Err("Release anchor requires an absolute build-input path".into());
        }
        println!("cargo::rerun-if-changed={text}");
        let file = std::fs::File::open(&path)?;
        if !file.metadata()?.is_file() {
            return Err("Release anchor must be a regular file".into());
        }
        file.take(BLOB_BYTES as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() != BLOB_BYTES
            || bytes[..HEADER.len()] != HEADER
            || bytes[HEADER.len()..HEADER.len() + EXPONENT.len()] != EXPONENT
        {
            return Err(
                "Release anchor is not an RSA-3072 CNG public blob with exponent 65537".into()
            );
        }
    }
    std::fs::write(output, bytes)?;
    Ok(())
}
