// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise hostile wire input and borrowed matching facts at the parser owner.
use super::*;

fn wire(build: &str, entries: &[(u8, &str, &str)]) -> Vec<u8> {
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&[4; 32]);
    bytes.push(build.len() as u8);
    bytes.extend_from_slice(build.as_bytes());
    bytes.extend_from_slice(&0x8664_u16.to_le_bytes());
    bytes.push(entries.len() as u8);
    for &(roles, pe, pdb) in entries {
        bytes.extend_from_slice(&[roles, pe.len() as u8]);
        bytes.extend_from_slice(pe.as_bytes());
        bytes.push(pdb.len() as u8);
        bytes.extend_from_slice(pdb.as_bytes());
        bytes.extend_from_slice(&1_u64.to_le_bytes());
        bytes.extend_from_slice(&[1; 32]);
        bytes.extend_from_slice(&2_u64.to_le_bytes());
        bytes.extend_from_slice(&[2; 32]);
        bytes.extend_from_slice(&4096_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&[3; 16]);
        bytes.extend_from_slice(&1_u32.to_le_bytes());
    }
    bytes
}
fn valid() -> Vec<u8> {
    wire("k1_actual", &[(7, "sugarsubstitute.exe", "sugarsubstitute.pdb")])
}

#[test]
fn exact_matching_facts_borrow_the_original_ingress() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = valid();
    let parsed = Manifest::parse(&bytes)?;
    assert_eq!(parsed.key_id, [4; 32]);
    assert_eq!(parsed.version, 1);
    assert_eq!(
        parsed.capture_pairs().err().ok_or("expected manifest rejection")?,
        ManifestError::UnknownSchema
    );
    assert_eq!(parsed.build, "k1_actual");
    let pair = parsed.role_pair().ok_or("fixture manifest record absent")?;
    assert_eq!(pair.roles, 7);
    assert_eq!(pair.pe_label, "sugarsubstitute.exe");
    assert_eq!(pair.pdb_label, "sugarsubstitute.pdb");
    assert_eq!((pair.pe_length, pair.pdb_length, pair.image_size), (1, 2, 4096));
    assert_eq!((pair.pe_sha256, pair.pdb_sha256), ([1; 32], [2; 32]));
    assert_eq!((pair.timestamp, pair.checksum), (0, 0));
    assert_eq!((pair.pdb_guid, pair.pdb_age), ([3; 16], 1));
    assert!(std::ptr::eq(
        pair,
        parsed.find("sugarsubstitute.exe").ok_or("fixture manifest record absent")?
    ));
    assert!(parsed.find("unknown.dll").is_none());
    assert_eq!(parsed.build.as_ptr(), bytes[54..].as_ptr());
    Ok(())
}

fn capture(entries: &[(u8, &str, &str)]) -> Vec<u8> {
    let mut bytes = wire("k1_actual", entries);
    bytes[..16].copy_from_slice(CAPTURE_MAGIC);
    bytes[16..20].copy_from_slice(&2_u32.to_le_bytes());
    bytes
}
fn capture_entries() -> [(u8, &'static str, &'static str); 3] {
    [
        (20, "crashpad_handler.exe", "crashpad_handler.pdb"),
        (7, "sugarsubstitute.exe", "sugarsubstitute.pdb"),
        (12, "sugarsubstitute_crashpad_client.dll", "sugarsubstitute_crashpad_client.pdb"),
    ]
}

#[test]
fn schema2_capture_records_borrow_one_manifest_with_distinct_fixed_roles()
-> Result<(), Box<dyn std::error::Error>> {
    let bytes = capture(&capture_entries());
    let parsed = Manifest::parse(&bytes)?;
    let (client, handler) = parsed.capture_pairs()?;
    assert_eq!(parsed.version, 2);
    assert_eq!((client.roles, handler.roles), (12, 20));
    assert_eq!(client.pe_label, "sugarsubstitute_crashpad_client.dll");
    assert_eq!(handler.pe_label, "crashpad_handler.exe");
    assert!(std::ptr::eq(
        client,
        parsed.find(client.pe_label).ok_or("fixture manifest record absent")?
    ));
    assert!(std::ptr::eq(
        handler,
        parsed.find(handler.pe_label).ok_or("fixture manifest record absent")?
    ));
    assert_eq!(
        parsed.role_pair().ok_or("fixture manifest record absent")?.pe_label,
        "sugarsubstitute.exe"
    );
    Ok(())
}

#[test]
fn schema2_capture_selection_rejects_missing_roles_and_schema1_has_no_capture()
-> Result<(), Box<dyn std::error::Error>> {
    for entries in [&capture_entries()[..2], &capture_entries()[1..]] {
        let bytes = capture(entries);
        assert_eq!(
            Manifest::parse(&bytes)?.capture_pairs().err().ok_or("expected manifest rejection")?,
            ManifestError::RoleMismatch
        );
    }
    assert!(Manifest::parse(&wire("k1", &capture_entries())).is_err());
    let bytes = valid();
    assert_eq!(
        Manifest::parse(&bytes)?.capture_pairs().err().ok_or("expected manifest rejection")?,
        ManifestError::UnknownSchema
    );
    Ok(())
}

#[test]
fn schema2_rejects_wrong_leaf_pair_type_combined_roles_and_unknown_bits()
-> Result<(), Box<dyn std::error::Error>> {
    for (index, replacement) in [
        (0, (16, "other.exe", "crashpad_handler.pdb")),
        (0, (16, "crashpad_handler.exe", "other.pdb")),
        (0, (24, "crashpad_handler.exe", "crashpad_handler.pdb")),
        (0, (19, "crashpad_handler.exe", "crashpad_handler.pdb")),
        (2, (8, "sugarsubstitute_crashpad_client.exe", "sugarsubstitute_crashpad_client.pdb")),
        (2, (8, "sugarsubstitute_crashpad_client.dll", "other.pdb")),
        (2, (15, "sugarsubstitute_crashpad_client.dll", "sugarsubstitute_crashpad_client.pdb")),
        (2, (32, "sugarsubstitute_crashpad_client.dll", "sugarsubstitute_crashpad_client.pdb")),
    ] {
        let mut entries = capture_entries();
        entries[index] = replacement;
        assert!(Manifest::parse(&capture(&entries)).is_err(), "{replacement:?}");
    }
    Ok(())
}

#[test]
fn schema2_rejects_truncation_trailing_and_mismatched_magic_version()
-> Result<(), Box<dyn std::error::Error>> {
    let bytes = capture(&capture_entries());
    for length in 0..bytes.len() {
        assert!(Manifest::parse(&bytes[..length]).is_err(), "prefix {length}");
    }
    let mut mismatched = bytes.clone();
    mismatched[16] = 1;
    assert_eq!(
        Manifest::parse(&mismatched).err().ok_or("expected manifest rejection")?,
        ManifestError::UnknownSchema
    );
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        Manifest::parse(&trailing).err().ok_or("expected manifest rejection")?,
        ManifestError::Trailing
    );
    Ok(())
}

#[test]
fn every_truncated_prefix_trailing_byte_and_oversized_ingress_rejects()
-> Result<(), Box<dyn std::error::Error>> {
    let bytes = valid();
    for length in 0..bytes.len() {
        assert!(Manifest::parse(&bytes[..length]).is_err(), "prefix {length}");
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        Manifest::parse(&trailing).err().ok_or("expected manifest rejection")?,
        ManifestError::Trailing
    );
    assert_eq!(
        Manifest::parse(&vec![0; MAX_MANIFEST_BYTES + 1])
            .err()
            .ok_or("expected manifest rejection")?,
        ManifestError::Bound
    );
    Ok(())
}

#[test]
fn unknown_magic_schema_kind_architecture_and_counts_reject()
-> Result<(), Box<dyn std::error::Error>> {
    let original = valid();
    let machine = 54 + usize::from(original[53]);
    for (offset, value) in
        [(0, 0), (16, 2), (20, 2), (machine, 0), (machine + 2, 0), (machine + 2, 17)]
    {
        let mut changed = original.clone();
        changed[offset] = value;
        assert!(Manifest::parse(&changed).is_err(), "offset {offset}");
    }
    Ok(())
}

#[test]
fn unsafe_leaf_syntax_roles_and_nonascii_build_reject_before_selection()
-> Result<(), Box<dyn std::error::Error>> {
    for pe in [
        "CON.exe", "con.exe", "com1.exe", "lpt9.exe", "../a.exe", "A.exe", "a..exe", "a.exe ",
        "a:p.exe", "a.dll", ".exe",
    ] {
        assert!(Manifest::parse(&wire("k1", &[(7, pe, "a.pdb")])).is_err(), "{pe}");
    }
    for roles in [0, 8, 255] {
        assert!(Manifest::parse(&wire("k1", &[(roles, "a.exe", "a.pdb")])).is_err());
    }
    for build in ["", "k1/other", "\u{00e9}"] {
        assert!(Manifest::parse(&wire(build, &[(7, "a.exe", "a.pdb")])).is_err());
    }
    assert!(Manifest::parse(&wire(&"x".repeat(65), &[(7, "a.exe", "a.pdb")])).is_err());
    assert!(Manifest::parse(&wire("k1", &[(7, "a.exe", "a.exe")])).is_err());
    assert!(
        Manifest::parse(&wire("k1", &[(7, &format!("{}.exe", "x".repeat(125)), "a.pdb")])).is_err()
    );
    Ok(())
}

#[test]
fn absent_lengths_image_pdb_guid_and_age_reject() -> Result<(), Box<dyn std::error::Error>> {
    let original = wire("k1", &[(7, "a.exe", "a.pdb")]);
    let facts = 54 + 2 + 3 + 2 + 5 + 1 + 5;
    for (offset, width) in
        [(facts, 8), (facts + 40, 8), (facts + 80, 4), (facts + 92, 16), (facts + 108, 4)]
    {
        let mut changed = original.clone();
        changed[offset..offset + width].fill(0);
        assert_eq!(
            Manifest::parse(&changed).err().ok_or("expected manifest rejection")?,
            ManifestError::InvalidField
        );
    }
    Ok(())
}

#[test]
fn missing_split_ambiguous_and_unordered_roles_or_aliases_reject()
-> Result<(), Box<dyn std::error::Error>> {
    for entries in [
        vec![(4, "a.exe", "a.pdb")],
        vec![(1, "a.exe", "a.pdb"), (2, "z.exe", "z.pdb")],
        vec![(7, "a.exe", "a.pdb"), (7, "z.exe", "z.pdb")],
        vec![(7, "a.exe", "a.pdb"), (4, "z.exe", "a.pdb")],
        vec![(7, "z.exe", "z.pdb"), (4, "a.exe", "a.pdb")],
        vec![(7, "a.exe", "a.pdb"), (4, "a.exe", "z.pdb")],
    ] {
        assert!(Manifest::parse(&wire("k1", &entries)).is_err(), "{entries:?}");
    }
    Ok(())
}

#[test]
fn sixteen_maximum_pairs_keep_the_inventory_bounded_and_exact()
-> Result<(), Box<dyn std::error::Error>> {
    let labels: Vec<_> = (0..16)
        .map(|index| {
            (
                format!("a{index:02}_{}.exe", "x".repeat(119)),
                format!("a{index:02}_{}.pdb", "x".repeat(119)),
            )
        })
        .collect();
    let entries: Vec<_> = labels
        .iter()
        .enumerate()
        .map(|(index, (pe, pdb))| (if index == 0 { 7 } else { 4 }, pe.as_str(), pdb.as_str()))
        .collect();
    let bytes = wire(&"b".repeat(64), &entries);
    assert!(bytes.len() < 8192);
    let parsed = Manifest::parse(&bytes)?;
    assert_eq!(parsed.count, 16);
    assert_eq!(
        parsed.find(&labels[15].0).ok_or("fixture manifest record absent")?.pdb_label,
        labels[15].1
    );
    let mut excess = entries;
    excess.push((4, "z.exe", "z.pdb"));
    assert_eq!(
        Manifest::parse(&wire("k1", &excess)).err().ok_or("expected manifest rejection")?,
        ManifestError::Bound
    );
    Ok(())
}

#[cfg(feature = "local-k1-symbol-trust")]
#[test]
fn signed_build_matches_only_the_compiled_source_inventory_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let bytes = wire(
        super::super::local_k1_build::BUILD,
        &[(7, "sugarsubstitute.exe", "sugarsubstitute.pdb")],
    );
    Manifest::parse(&bytes)?.require_compiled_build()?;
    let wrong =
        wire("k1_other_signed_generation", &[(7, "sugarsubstitute.exe", "sugarsubstitute.pdb")]);
    assert_eq!(
        Manifest::parse(&wrong)?
            .require_compiled_build()
            .err()
            .ok_or("Different signed build accepted")?,
        ManifestError::BuildMismatch
    );
    Ok(())
}
