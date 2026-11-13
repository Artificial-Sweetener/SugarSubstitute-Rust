// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve established wire bytes while requiring complete authenticated trial-transfer facts.
use super::*;

fn frame(trial: Option<InstallationTrialCustody>) -> Result<BackendCustodyFrame, Box<dyn Error>> {
    Ok(BackendCustodyFrame {
        key: key()?,
        nonce: 2,
        command: BackendCustodyCommand::Establish {
            parent: 40,
            pid: 9,
            birth: 20,
            containment: 44,
            trial,
        },
    })
}
fn trial() -> InstallationTrialCustody {
    InstallationTrialCustody { run: 48, snapshot: 52, until_ticks: 300, frequency: 10 }
}

#[test]
fn optional_trial_uses_distinct_tag_without_changing_existing_establishment_bytes() -> TestResult {
    let original = frame(None)?.encode()?;
    assert_eq!(original[10], 2);
    assert!(original[112..].iter().all(|byte| *byte == 0));
    let extended = frame(Some(trial()))?;
    let encoded = extended.encode()?;
    assert_eq!(encoded.len(), 384);
    assert_eq!(encoded[10], 23);
    assert_eq!(BackendCustodyFrame::decode(&encoded)?, extended);
    assert_eq!(&original[16..112], &encoded[16..112]);
    Ok(())
}

#[test]
fn partial_or_downgraded_trial_enrollment_cannot_decode_as_ordinary_establishment() -> TestResult {
    let encoded = frame(Some(trial()))?.encode()?;
    let mut downgraded = encoded;
    downgraded[10] = 2;
    assert!(BackendCustodyFrame::decode(&downgraded).is_err());
    for start in [112, 120, 128, 136] {
        let mut partial = encoded;
        partial[start..start + 8].fill(0);
        assert!(BackendCustodyFrame::decode(&partial).is_err());
    }
    let mut changed = encoded;
    changed[144] = 1;
    assert!(BackendCustodyFrame::decode(&changed).is_err());
    Ok(())
}

#[test]
fn trial_handles_must_be_distinct_from_each_other_and_native_parent_outputs() -> TestResult {
    for raw in [0, 40, 44, 52, u64::MAX, u64::MAX - 1] {
        assert!(frame(Some(InstallationTrialCustody { run: raw, ..trial() }))?.encode().is_err());
    }
    for raw in [0, 40, 44, 48, u64::MAX, u64::MAX - 1] {
        assert!(
            frame(Some(InstallationTrialCustody { snapshot: raw, ..trial() }))?.encode().is_err()
        );
    }
    for (until_ticks, frequency) in [(-1, 10), (300, -1), (0, 10), (300, 0)] {
        assert!(
            frame(Some(InstallationTrialCustody { until_ticks, frequency, ..trial() }))?
                .encode()
                .is_err()
        );
    }
    Ok(())
}
