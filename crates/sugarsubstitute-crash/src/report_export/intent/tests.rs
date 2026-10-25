// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject ambiguous selected destinations without performing any native effects.
use super::*;
use std::error::Error;
use sugarsubstitute_contracts::{incident::*, lifecycle::*};
fn binding() -> Result<ReportBinding, Box<dyn Error>> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    Ok(ReportBinding::new(
        IncidentKey::new(IncidentSubject::Supervisor(owner)),
        IncidentRevision::new(2)?,
        ReportSessionId::new(1)?,
    ))
}
fn selection<'a>(destination: &'a [u8]) -> Result<DiagnosticSelection<'a>, Box<dyn Error>> {
    Ok(DiagnosticSelection {
        id: DiagnosticRequestId::new(1)?,
        selection: ExportSelection::TextOnly,
        overwrite: false,
        destination,
    })
}
fn units(path: &str) -> Vec<u8> {
    path.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
#[test]
fn export_intent_preserves_report_binding_selection_and_request() -> Result<(), Box<dyn Error>> {
    let bytes = units("D:\\糖\\report😀.txt");
    let intent = ReportExportIntent::new(binding()?, selection(&bytes)?)?;
    assert_eq!(intent.binding(), binding()?);
    assert_eq!(intent.request().get(), 1);
    assert_eq!(intent.selection(), ExportSelection::TextOnly);
    Ok(())
}
#[test]
fn export_destination_rejects_device_relative_ads_ambiguous_and_reserved_paths()
-> Result<(), Box<dyn Error>> {
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
        assert!(ReportExportIntent::new(binding()?, selection(&bytes)?).is_err(), "{path:?}");
    }
    Ok(())
}
#[test]
fn export_destination_rejects_bounds_and_malformed_utf16_before_effects()
-> Result<(), Box<dyn Error>> {
    let mut bytes = units("D:\\r");
    bytes.extend_from_slice(&0xd800_u16.to_le_bytes());
    assert!(ReportExportIntent::new(binding()?, selection(&bytes)?).is_err());
    bytes = units("D:\\r.txt");
    bytes.push(0);
    assert!(ReportExportIntent::new(binding()?, selection(&bytes)?).is_err());
    for path in [
        format!("D:\\{}.txt", "a".repeat(256)),
        format!("D:\\{}r.txt", "a\\".repeat(64)),
        format!("D:\\{}r.txt", "long\\".repeat(220)),
    ] {
        assert!(ReportExportIntent::new(binding()?, selection(&units(&path))?).is_err());
    }
    Ok(())
}
