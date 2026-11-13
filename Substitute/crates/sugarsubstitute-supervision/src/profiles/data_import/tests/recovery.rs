// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Canonical retained recovery/original pairs remain lifecycle metadata rather than copied authority.
use super::*;

fn publish_pair(
    fixture: &Fixture,
    claim: &str,
    record_name: &str,
    bytes: &[u8],
    context: &JobContext,
) -> io::Result<()> {
    protection::with(context, |owner| {
        use std::io::Write;
        let file = owner.file(&fixture.source.path().join(claim), context)?;
        files::sync(&file, context)?;
        owner.close_file(file)?;
        let mut file = owner.file(&fixture.source.path().join(record_name), context)?;
        file.write_all(bytes)?;
        files::sync(&file, context)?;
        owner.close_file(file)
    })
}

#[test]
fn recovery_pairs_preserve_original_v2_held_rows_and_reject_unproduced_legacy_rows() -> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        let original = record::Record {
            run: RunKey {
                owner: CoordinatorKey {
                    namespace: NamespaceId::new([9; 32]),
                    epoch: CoordinatorEpoch::new([4; 16])?,
                },
                instance: InstanceId::new(8)?,
                generation: RunGeneration::new(3)?,
            },
            state: record::State::Held,
            ..fixture.row
        };
        let stem = locations::recovered(original.run);
        let claim = format!("{stem}.claim");
        let recovered = format!("{stem}.record");
        let bytes =
            record::custody::CustodiedRecord { original, custodian: 11, registry_process: 12 }
                .encode();
        publish_pair(&fixture, &claim, &recovered, &bytes, context)?;
        with_released(&fixture.source, &[fixture.row.run.owner.namespace], context, |source| {
            assert!(source.excluded_root_controls().contains(&claim.as_str()));
            assert!(source.excluded_root_controls().contains(&recovered.as_str()));
            assert_eq!(source.namespace(), fixture.row.run.owner.namespace);
            Ok(())
        })?;
        assert_eq!(std::fs::read(fixture.source.path().join(&recovered))?, bytes);
        for unsupported in [
            original.encode(),
            record::custody::CustodiedRecord {
                original: record::Record { state: record::State::Released, ..original },
                custodian: 11,
                registry_process: 12,
            }
            .encode(),
        ] {
            std::fs::write(fixture.source.path().join(&recovered), unsupported)?;
            assert!(
                fixture.admit(context).is_err(),
                "A recovery filename requires the exact original v2 Held row"
            );
            assert_eq!(std::fs::read(fixture.source.path().join(&recovered))?, unsupported);
        }
        Ok(())
    })
}

#[test]
fn recovery_original_backup_preserves_the_released_v1_pair_and_rejects_other_shapes() -> TestResult
{
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        let original = fixture.row.encode();
        publish_pair(
            &fixture,
            locations::ORIGINAL_CLAIM,
            locations::ORIGINAL_RECORD,
            &original,
            context,
        )?;
        fixture.admit(context)?;
        for unsupported in [
            record::custody::CustodiedRecord {
                original: fixture.row,
                custodian: 11,
                registry_process: 12,
            }
            .encode(),
            record::Record { state: record::State::Held, ..fixture.row }.encode(),
        ] {
            std::fs::write(fixture.source.path().join(locations::ORIGINAL_RECORD), unsupported)?;
            assert!(
                fixture.admit(context).is_err(),
                "An original-v1 backup requires the exact original v1 Released shape"
            );
            assert_eq!(
                std::fs::read(fixture.source.path().join(locations::ORIGINAL_RECORD))?,
                unsupported
            );
        }
        Ok(())
    })
}

#[test]
fn recovery_released_v2_requires_its_original_independent_custody_exclusion() -> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        let bytes = record::custody::CustodiedRecord {
            original: fixture.row,
            custodian: 11,
            registry_process: 12,
        }
        .encode();
        std::fs::write(fixture.source.path().join(locations::RECORD), bytes)?;
        fixture.admit(context)?;
        std::fs::remove_file(fixture.source.path().join(locations::CUSTODY))?;
        assert!(
            fixture.admit(context).is_err(),
            "A v2 producer always creates the independent witness exclusion before publishing its row"
        );
        assert_eq!(std::fs::read(fixture.source.path().join(locations::RECORD))?, bytes);
        Ok(())
    })
}
