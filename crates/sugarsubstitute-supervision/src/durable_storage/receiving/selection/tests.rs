// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify fixed selected path bounds without granting native authority.
use super::*;
fn units(path: &str) -> Vec<u8> {
    path.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
#[test]
fn selection_preserves_unicode_and_exact_sibling_identity() -> Result<(), Box<dyn std::error::Error>>
{
    let path = "D:\\糖\\report😀.txt";
    let selection = ReportFileSelection::new(&units(path), true, [17, 29])?;
    assert_eq!(&selection.path[..selection.length], path.encode_utf16().collect::<Vec<_>>());
    assert_eq!(selection.identity, [17, 29]);
    assert!(selection.overwrite);
    assert_eq!(selection.directories, 2);
    Ok(())
}
#[test]
fn export_destination_rejects_device_relative_ads_ambiguous_and_reserved_paths()
-> Result<(), Box<dyn std::error::Error>> {
    for path in [
        "",
        "report.txt",
        "D:report.txt",
        "\\\\server\\share\\report.txt",
        "\\\\?\\D:\\r.txt",
        "D:\\",
        "D:\\a\\..\\r.txt",
        "D:\\.\\r.txt",
        "D:\\r.txt:stream",
        "D:\\a\\r.txt.",
        "D:\\r.txt ",
        "D:\\a\\\\r.txt",
        "D:\\NUL",
        "D:\\con.txt",
        "D:\\COM9.bin",
        "D:\\LPT1.txt",
        "D:\\CONIN$",
        "D:\\conout$.txt",
        "D:\\COM¹.txt",
        "D:\\a/r.txt",
        "D:\\r\0.txt",
    ] {
        let bytes = units(path);
        assert!(ReportFileSelection::new(&bytes, false, [1, 1]).is_err(), "{path:?}");
    }
    Ok(())
}
#[test]
fn export_destination_rejects_bounds_and_malformed_utf16_before_effects()
-> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = units("D:\\r");
    bytes.extend_from_slice(&0xd800_u16.to_le_bytes());
    assert!(ReportFileSelection::new(&bytes, false, [1, 1]).is_err());
    bytes = units("D:\\r.txt");
    bytes.push(0);
    assert!(ReportFileSelection::new(&bytes, false, [1, 1]).is_err());
    for path in [
        format!("D:\\{}.txt", "a".repeat(256)),
        format!("D:\\{}r.txt", "a\\".repeat(64)),
        format!("D:\\{}r.txt", "long\\".repeat(220)),
    ] {
        assert!(ReportFileSelection::new(&units(&path), false, [1, 1]).is_err());
    }
    Ok(())
}
