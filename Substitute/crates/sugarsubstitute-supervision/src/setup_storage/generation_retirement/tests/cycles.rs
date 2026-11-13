// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Disk use stays flat across many updates, whatever interrupts or holds old versions.
use super::*;

#[test]
fn disk_use_stays_flat_through_rejected_superseded_held_and_interrupted_updates() -> TestResult {
    test_worker::run_reserved(RESERVATION, |context, _| {
        let (mut installation, first) = accepted_one(context)?;
        let releases = installation.releases();
        let mut previous = first.clone();
        let mut older: Option<Generation> = None;
        let mut largest = 0;
        for cycle in 0..6_u64 {
            let base = 2 + 3 * cycle;
            // A release that crashes on its first start is rejected.
            let bad = generation(base)?;
            largest = largest.max(write_generation(&releases, &bad)?);
            installation.prepare(&bad, context)?;
            installation.settle(Some(TrialVerdict::Bad(BadTrial::Crashed)), context)?;
            // A newer release supersedes a candidate before it ever starts, then is accepted.
            let (superseded, next) = (generation(base + 1)?, generation(base + 2)?);
            for generation in [&superseded, &next] {
                largest = largest.max(write_generation(&releases, generation)?);
                installation.prepare(generation, context)?;
            }
            installation.settle(None, context)?;
            let pointer = Pointer::naming(&[&next, &previous]);
            let held = match (cycle % 3, older.as_ref()) {
                // An antivirus scan holds the oldest version's executable during the turn.
                (1, Some(old)) => Some(std::fs::File::open(
                    releases.join(old.id.as_str()).join("sugarsubstitute.exe"),
                )?),
                // A power cut after the tombstone rename, with part of the deletion done.
                (2, Some(old)) => {
                    let tombstone = format!(".retired-{cycle:032x}.{}", old.id.as_str());
                    std::fs::rename(releases.join(old.id.as_str()), releases.join(&tombstone))?;
                    std::fs::remove_file(releases.join(&tombstone).join("sugarsubstitute.exe"))?;
                    None
                }
                _ => None,
            };
            let report = installation.finished(&pointer, context)?;
            assert_eq!((report.retained, report.preserved), (0, 0));
            if let Some(file) = held {
                assert_eq!(report.in_use, 1);
                drop(file);
                // The next turn, at the next launch, removes it once the scan lets go.
                assert_eq!(installation.finished(&pointer, context)?.removed, 1);
            }
            assert_eq!(installation.names()?, [previous.id.as_str(), next.id.as_str()]);
            assert!(folder_bytes(&releases)? <= 2 * largest);
            older = Some(previous);
            previous = next;
        }
        Ok(())
    })
}

#[test]
fn an_updated_version_leaves_neither_its_receipt_nor_an_interrupted_receipt_stage() -> TestResult {
    use crate::setup_storage::{UPDATE_RECEIPT_FILE, UPDATE_RECEIPT_STAGE};
    test_worker::run_reserved(RESERVATION, |context, _| {
        let (mut installation, first) = accepted_one(context)?;
        let mut previous = first;
        for number in 2..=4 {
            let next = generation(number)?;
            write_generation(&installation.releases(), &next)?;
            // Preparation wrote the offline receipt; a power cut during a retried receipt
            // write left its stage beside it.
            let folder = installation.releases().join(next.id.as_str());
            std::fs::write(folder.join(UPDATE_RECEIPT_FILE), b"receipt")?;
            std::fs::write(folder.join(format!("{UPDATE_RECEIPT_STAGE}{number:032x}")), b"torn")?;
            installation.prepare(&next, context)?;
            installation.settle(None, context)?;
            let report = installation.finished(&Pointer::naming(&[&next, &previous]), context)?;
            assert_eq!((report.retained, report.preserved, report.unknown), (0, 0, 0));
            assert_eq!(installation.names()?, [previous.id.as_str(), next.id.as_str()]);
            previous = next;
        }
        Ok(())
    })
}
