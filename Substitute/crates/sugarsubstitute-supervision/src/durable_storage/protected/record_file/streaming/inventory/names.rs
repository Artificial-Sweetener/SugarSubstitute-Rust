// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate the complete canonical leaf grammar before inventory observation.
use super::*;
use sugarsubstitute_contracts::incident::IncidentRevision;

/// Matching data from the complete canonical grammar; native checks grant custody.
pub(super) struct ClassifiedName {
    pub(super) digest: [u8; 32],
    pub(super) class: DiagnosticEntryClass,
    pub(super) payload_group: Option<[u8; 32]>,
}
/// Parse existing names into subject and generation matching data without native authority.
pub(super) fn classify(name: &str) -> ClassifiedName {
    let Some((digest, rest)) = name.split_once('.') else {
        return unknown();
    };
    let Some(digest) = layout::decode_digest(digest) else {
        return unknown();
    };
    let (class, revision) = match rest {
        "seed" => (DiagnosticEntryClass::Seed, None),
        "summary" => (DiagnosticEntryClass::Summary, None),
        "state" => (DiagnosticEntryClass::StoreState, None),
        "journal" => (DiagnosticEntryClass::Journal, None),
        "quarantine" => (DiagnosticEntryClass::Quarantine, None),
        _ => {
            if let Some(revision) = payload_revision(rest) {
                (DiagnosticEntryClass::Payload, Some(revision))
            } else if let Some(target) = stage_target(rest) {
                (DiagnosticEntryClass::Stage, payload_revision(target))
            } else {
                (DiagnosticEntryClass::Unknown, None)
            }
        }
    };
    ClassifiedName {
        digest,
        class,
        payload_group: revision.map(|revision| layout::payload_group(digest, revision)),
    }
}
fn unknown() -> ClassifiedName {
    ClassifiedName { digest: [0; 32], class: DiagnosticEntryClass::Unknown, payload_group: None }
}
fn payload_revision(rest: &str) -> Option<IncidentRevision> {
    let value = rest.strip_prefix("p-")?.strip_suffix(".payload")?;
    let (revision, kind) = value.split_once('-')?;
    if revision.len() != 16
        || !revision.bytes().all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
        || !matches!(kind, "1" | "2" | "3" | "4" | "5" | "6")
    {
        return None;
    }
    IncidentRevision::new(u64::from_str_radix(revision, 16).ok()?).ok()
}
fn stage_target(rest: &str) -> Option<&str> {
    let value = rest.strip_suffix(".stage")?;
    let (value, nonce) = value.rsplit_once('.')?;
    let (target, operation) = value.rsplit_once('.')?;
    ((matches!(target, "seed" | "summary" | "state" | "journal")
        || payload_revision(target).is_some())
        && layout::decode_digest(operation).is_some()
        && nonce.len() == 32
        && nonce.bytes().all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
        && nonce.bytes().any(|value| value != b'0'))
    .then_some(())?;
    Some(target)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_generation_canonical_payload_and_stage_groups_preserve_subject_binding()
    -> Result<(), Box<dyn std::error::Error>> {
        let digest = [3; 32];
        let hex = super::super::super::super::hex(&digest);
        let group = layout::payload_group(digest, IncidentRevision::new(7)?);
        for kind in 1..=6 {
            let leaf = format!("{hex}.p-0000000000000007-{kind}.payload");
            for name in
                [leaf.clone(), format!("{leaf}.{}.{}.stage", "04".repeat(32), "05".repeat(16))]
            {
                let observed = classify(&name);
                assert_eq!(observed.digest, digest);
                assert_eq!(observed.payload_group, Some(group));
                assert_eq!(
                    observed.class,
                    if name.ends_with(".stage") {
                        DiagnosticEntryClass::Stage
                    } else {
                        DiagnosticEntryClass::Payload
                    }
                );
            }
        }
        assert_ne!(
            classify(&format!("{hex}.p-0000000000000008-1.payload")).payload_group,
            Some(group)
        );
        assert_eq!(
            classify(&format!("{hex}.summary.{}.{}.stage", "04".repeat(32), "05".repeat(16)))
                .payload_group,
            None
        );
        Ok(())
    }
    #[test]
    fn diagnostic_generation_noncanonical_payload_stage_never_projects_a_group() {
        let hex = "03".repeat(32);
        for target in [
            "p-0000000000000000-1.payload",
            "p-0000000000000001-7.payload",
            "p-000000000000000A-1.payload",
            "p-1-1.payload",
            "p-0000000000000001-1.payload.extra",
        ] {
            for name in [
                format!("{hex}.{target}"),
                format!("{hex}.{target}.{}.{}.stage", "04".repeat(32), "05".repeat(16)),
            ] {
                let observed = classify(&name);
                assert_eq!(observed.class, DiagnosticEntryClass::Unknown);
                assert_eq!(observed.payload_group, None);
            }
        }
        for (operation, nonce) in
            [("00".repeat(32), "05".repeat(16)), ("04".repeat(32), "00".repeat(16))]
        {
            let observed =
                classify(&format!("{hex}.p-0000000000000001-1.payload.{operation}.{nonce}.stage"));
            assert_eq!(observed.class, DiagnosticEntryClass::Unknown);
            assert_eq!(observed.payload_group, None);
        }
    }
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
            assert_eq!(classify(&format!("{digest}.{rest}")).class, DiagnosticEntryClass::Unknown);
        }
        assert_eq!(
            classify(&format!("{digest}.p-0000000000000001-6.payload")).class,
            DiagnosticEntryClass::Payload
        );
        assert_eq!(
            classify(&format!(
                "{digest}.summary.{}.{}.stage",
                super::super::super::super::hex(&[4; 32]),
                super::super::super::super::hex(&[5; 16])
            ))
            .class,
            DiagnosticEntryClass::Stage
        );
    }
}
