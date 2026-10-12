// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Test complete matching identity and immutable UTC/revision boundaries.
use super::*;

#[test]
fn failed_run_identity_preserves_every_scope_field() -> Result<(), Box<dyn std::error::Error>> {
    let key = desktop()?;
    let IncidentSubject::Desktop(run) = key.subject() else {
        return Err("desktop subject changed".into());
    };
    let variants = [
        RunKey { instance: InstanceId::new(5)?, ..run },
        RunKey { generation: RunGeneration::new(5)?, ..run },
        RunKey {
            owner: CoordinatorKey { epoch: CoordinatorEpoch::new([3; 16])?, ..run.owner },
            ..run
        },
        RunKey {
            owner: CoordinatorKey { namespace: NamespaceId::new([4; 32]), ..run.owner },
            ..run
        },
    ];
    for other in variants {
        assert_ne!(key, IncidentKey::new(IncidentSubject::Desktop(other)));
    }
    assert_ne!(key, IncidentKey::new(IncidentSubject::Supervisor(run.owner)));
    assert_eq!(key.subject().coordinator(), run.owner);
    assert_eq!(key, IncidentKey::new(IncidentSubject::Desktop(run)));
    Ok(())
}

#[test]
fn revision_zero_and_exhaustion_never_wrap() -> Result<(), IncidentError> {
    assert_eq!(IncidentRevision::new(0), Err(IncidentError::InvalidIdentity));
    let first = IncidentRevision::new(1)?;
    assert_eq!(first.next()?.get(), 2);
    assert_eq!(first.get(), 1);
    let last = IncidentRevision::new(u64::MAX)?;
    assert_eq!(last.next(), Err(IncidentError::RevisionExhausted));
    assert_eq!(last.get(), u64::MAX);
    Ok(())
}

#[test]
fn build_annotation_boundary_rejects_native_path_and_unbounded_input() -> Result<(), IncidentError>
{
    let maximum = "a".repeat(64);
    assert_eq!(BuildIdentity::new(&maximum)?.as_str(), maximum);
    assert_eq!(BuildIdentity::new("A9_release-test")?.as_str(), "A9_release-test");
    for invalid in ["", "E:\\private", "../release", "name.json", "a\0b", "日本語", "two words"]
    {
        assert_eq!(BuildIdentity::new(invalid), Err(IncidentError::InvalidText));
    }
    assert_eq!(BuildIdentity::new(&"a".repeat(65)), Err(IncidentError::InvalidText));
    Ok(())
}

#[test]
fn utc_bounds_and_acknowledgement_order_are_explicit() -> Result<(), IncidentError> {
    assert_eq!(IncidentUtc::from_unix_seconds(-1), Err(IncidentError::InvalidTime));
    assert_eq!(IncidentUtc::from_unix_seconds(0)?.unix_seconds(), 0);
    assert_eq!(IncidentUtc::from_unix_seconds(253_402_300_799)?.unix_seconds(), 253_402_300_799);
    assert_eq!(IncidentUtc::from_unix_seconds(253_402_300_800), Err(IncidentError::InvalidTime));
    let occurred = IncidentUtc::from_unix_seconds(100)?;
    assert_eq!(RetentionFacts::new(occurred, None)?.acknowledged(), None);
    assert_eq!(RetentionFacts::new(occurred, Some(occurred))?.acknowledged(), Some(occurred));
    assert_eq!(
        RetentionFacts::new(occurred, Some(IncidentUtc::from_unix_seconds(99)?)),
        Err(IncidentError::InvalidTime)
    );
    assert_eq!(RetentionFacts::new(occurred, None)?.occurred(), occurred);
    Ok(())
}

#[test]
fn dump_and_pdb_matching_identities_require_present_values() -> Result<(), IncidentError> {
    assert_eq!(DumpId::new([0; 16]), Err(IncidentError::InvalidIdentity));
    assert_eq!(DumpId::new([1; 16])?.bytes(), &[1; 16]);
    assert_eq!(PdbIdentity::new([0; 16], 1), Err(IncidentError::InvalidIdentity));
    assert_eq!(PdbIdentity::new([1; 16], 0), Err(IncidentError::InvalidIdentity));
    let pdb = PdbIdentity::new([2; 16], u32::MAX)?;
    assert_eq!(pdb.guid(), &[2; 16]);
    assert_eq!(pdb.age(), u32::MAX);
    Ok(())
}

#[test]
fn symbol_labels_bound_utf8_and_exclude_native_path_syntax() -> Result<(), IncidentError> {
    assert_eq!(FileLabel::new(&"x".repeat(128))?.as_str().len(), 128);
    for invalid in ["", ".", "..", "dir/file.rs", "E:\\file.rs", "name\n.rs"] {
        assert_eq!(FileLabel::new(invalid), Err(IncidentError::InvalidText));
    }
    assert_eq!(FileLabel::new(&"x".repeat(129)), Err(IncidentError::InvalidText));
    let name = "λ".repeat(256);
    assert_eq!(SymbolName::new(&name)?.as_str(), name);
    assert_eq!(SymbolName::new(&"λ".repeat(257)), Err(IncidentError::InvalidText));
    for invalid in ["", "function\0name", "function\nname"] {
        assert_eq!(SymbolName::new(invalid), Err(IncidentError::InvalidText));
    }
    assert_eq!(
        SourceLine::new(FileLabel::new("source.rs")?, 0),
        Err(IncidentError::InvalidIdentity)
    );
    assert_eq!(SourceLine::new(FileLabel::new("source.rs")?, u32::MAX)?.line(), u32::MAX);
    Ok(())
}
