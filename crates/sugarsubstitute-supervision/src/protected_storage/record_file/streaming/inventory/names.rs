// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate the complete canonical leaf grammar before inventory observation.
use super::*;
pub(super) fn classify(name: &str) -> ([u8; 32], DiagnosticEntryClass) {
    let Some((digest, rest)) = name.split_once('.') else {
        return ([0; 32], DiagnosticEntryClass::Unknown);
    };
    let Some(digest) = layout::decode_digest(digest) else {
        return ([0; 32], DiagnosticEntryClass::Unknown);
    };
    let class = match rest {
        "seed" => DiagnosticEntryClass::Seed,
        "summary" => DiagnosticEntryClass::Summary,
        "state" => DiagnosticEntryClass::StoreState,
        "journal" => DiagnosticEntryClass::Journal,
        _ if payload(rest) => DiagnosticEntryClass::Payload,
        _ if stage(rest) => DiagnosticEntryClass::Stage,
        "quarantine" => DiagnosticEntryClass::Quarantine,
        _ => DiagnosticEntryClass::Unknown,
    };
    (digest, class)
}
fn payload(rest: &str) -> bool {
    let Some(value) = rest.strip_prefix("p-").and_then(|value| value.strip_suffix(".payload"))
    else {
        return false;
    };
    let Some((revision, kind)) = value.split_once('-') else {
        return false;
    };
    revision.len() == 16
        && revision.bytes().all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
        && u64::from_str_radix(revision, 16).is_ok_and(|revision| revision != 0)
        && matches!(kind, "1" | "2" | "3" | "4" | "5" | "6")
}
fn stage(rest: &str) -> bool {
    let Some(value) = rest.strip_suffix(".stage") else {
        return false;
    };
    let Some((value, nonce)) = value.rsplit_once('.') else {
        return false;
    };
    let Some((target, operation)) = value.rsplit_once('.') else {
        return false;
    };
    (matches!(target, "seed" | "summary" | "state" | "journal") || payload(target))
        && layout::decode_digest(operation).is_some()
        && nonce.len() == 32
        && nonce.bytes().all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
        && nonce.bytes().any(|value| value != b'0')
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_noncanonical_physical_suffixes_remain_unknown() {
        let digest = super::super::super::super::hex(&[3; 32]);
        for rest in [
            "p-0000000000000000-1.payload",
            "p-0000000000000001-7.payload",
            "p-1-1.payload",
            "unexpected.stage",
            "summary.bad.bad.stage",
            "../summary",
        ] {
            assert_eq!(classify(&format!("{digest}.{rest}")).1, DiagnosticEntryClass::Unknown);
        }
        assert_eq!(
            classify(&format!("{digest}.p-0000000000000001-6.payload")).1,
            DiagnosticEntryClass::Payload
        );
        assert_eq!(
            classify(&format!(
                "{digest}.summary.{}.{}.stage",
                super::super::super::super::hex(&[4; 32]),
                super::super::super::super::hex(&[5; 16])
            ))
            .1,
            DiagnosticEntryClass::Stage
        );
    }
}
