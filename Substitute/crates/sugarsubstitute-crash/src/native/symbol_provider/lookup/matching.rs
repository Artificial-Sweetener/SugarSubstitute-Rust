// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match the actual direct-loaded PDB against authenticated selected and captured identities.
use std::path::Path;
use sugarsubstitute_contracts::incident::PdbIdentity;
use windows::Win32::System::Diagnostics::Debug::{IMAGEHLP_MODULEW64, SymPdb};

/// Compare exact loaded facts after the caller authenticated the selected pair against the frame.
/// The native call loads that PDB directly with the captured base and size. Its match is
/// established by these actual identities; the PdbUnmatched observation grants no match authority.
pub(super) fn direct_pdb(
    actual: &IMAGEHLP_MODULEW64,
    expected: PdbIdentity,
    captured_base: u64,
    captured_size: u32,
    expected_path: &Path,
    loaded_path: &str,
) -> bool {
    actual.SymType == SymPdb
        && !actual.DbgUnmatched.as_bool()
        && actual.BaseOfImage == captured_base
        && actual.ImageSize == captured_size
        && guid_bytes(actual.PdbSig70) == *expected.guid()
        && actual.PdbAge == expected.age()
        && same_pdb_name(expected_path, loaded_path)
}
/// Normalize the scalar GUID's native little-endian fields to CodeView identity bytes.
fn guid_bytes(guid: windows::core::GUID) -> [u8; 16] {
    let mut out = [0; 16];
    out[..4].copy_from_slice(&guid.data1.to_le_bytes());
    out[4..6].copy_from_slice(&guid.data2.to_le_bytes());
    out[6..8].copy_from_slice(&guid.data3.to_le_bytes());
    out[8..].copy_from_slice(&guid.data4);
    out
}
/// Admit only the same absolute DOS path with its optional native verbatim prefix.
fn same_pdb_name(expected: &Path, actual: &str) -> bool {
    fn spelling(value: &str) -> &str {
        if let Some(dos) = value.strip_prefix(r"\\?\")
            && dos.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
            && dos.as_bytes().get(1..3) == Some(br":\")
        {
            return dos;
        }
        value
    }
    expected.to_str().is_some_and(|value| spelling(value).eq_ignore_ascii_case(spelling(actual)))
}
#[cfg(test)]
mod tests {
    use super::*;
    use windows::{Win32::System::Diagnostics::Debug::SymExport, core::BOOL};
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    /// Supply a complete native observation independently of its advisory unmatched flag.
    fn observed(unmatched: bool) -> IMAGEHLP_MODULEW64 {
        IMAGEHLP_MODULEW64 {
            BaseOfImage: 0x1000,
            ImageSize: 4096,
            SymType: SymPdb,
            PdbSig70: windows::core::GUID::from_u128(0x01010101010101010101010101010101),
            PdbAge: 2,
            PdbUnmatched: BOOL(i32::from(unmatched)),
            ..Default::default()
        }
    }
    #[test]
    fn direct_authenticated_pdb_uses_exact_loaded_identity_for_either_unmatched_observation()
    -> TestResult {
        let expected = PdbIdentity::new([1; 16], 2)?;
        for unmatched in [false, true] {
            assert!(direct_pdb(
                &observed(unmatched),
                expected,
                0x1000,
                4096,
                Path::new(r"\\?\E:\signed\SugarSubstitute.pdb"),
                r"E:\signed\SugarSubstitute.pdb"
            ));
        }
        Ok(())
    }
    #[test]
    fn direct_pdb_rejects_every_wrong_actual_identity_even_when_unmatched_flag_is_clear()
    -> TestResult {
        let expected = PdbIdentity::new([1; 16], 2)?;
        for unmatched in [false, true] {
            for wrong in 0..7 {
                let mut actual = observed(unmatched);
                let mut loaded = r"E:\signed\SugarSubstitute.pdb";
                match wrong {
                    0 => actual.BaseOfImage += 1,
                    1 => actual.ImageSize += 1,
                    2 => actual.PdbSig70.data1 ^= 1,
                    3 => actual.PdbAge += 1,
                    4 => actual.SymType = SymExport,
                    5 => actual.DbgUnmatched = BOOL(1),
                    _ => loaded = r"E:\foreign\SugarSubstitute.pdb",
                }
                assert!(
                    !direct_pdb(
                        &actual,
                        expected,
                        0x1000,
                        4096,
                        Path::new(r"E:\signed\SugarSubstitute.pdb"),
                        loaded
                    ),
                    "unmatched {unmatched} wrong field {wrong}"
                );
            }
        }
        Ok(())
    }
    #[test]
    fn loaded_pdb_spelling_allows_only_same_absolute_dos_path() {
        let expected = Path::new(r"\\?\E:\signed\SugarSubstitute.pdb");
        assert!(same_pdb_name(expected, r"E:\signed\SugarSubstitute.pdb"));
        assert!(same_pdb_name(expected, r"e:\SIGNED\SugarSubstitute.pdb"));
        assert!(!same_pdb_name(expected, r"E:\other\SugarSubstitute.pdb"));
        assert!(!same_pdb_name(expected, r"E:\signed\..\SugarSubstitute.pdb"));
    }
}
