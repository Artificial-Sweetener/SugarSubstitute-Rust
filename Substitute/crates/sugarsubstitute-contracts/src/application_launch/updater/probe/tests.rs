// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Probe correlation remains bounded data with strict native trial identities.
use super::*;
use crate::installation::{Generation, GenerationId, InstallationId, PackageDigest};

/// Build a valid opaque attempt without filesystem or process authority.
fn attempt() -> Result<Trial, crate::installation::Rejection> {
    Ok(Trial {
        installation: InstallationId::new("1".repeat(32))?,
        generation: Generation {
            id: GenerationId::new("2".repeat(32))?,
            package: PackageDigest::new("3".repeat(64))?,
            release: crate::installation::SignedRelease::new("1.0.0".to_owned(), 1)?,
        },
        revision: 7,
    })
}

#[test]
fn probe_correlation_round_trips_the_exact_attempt() -> Result<(), Box<dyn std::error::Error>> {
    let expected = attempt()?;
    let encoded = ProbeRequest::new(expected.clone())?.encode()?;
    assert!(encoded.len() <= MAX_PROBE_BYTES);
    assert_eq!(ProbeRequest::decode(encoded.as_bytes())?.attempt(), &expected);
    Ok(())
}

#[test]
fn malformed_oversized_duplicate_and_zero_revision_correlations_are_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let encoded = ProbeRequest::new(attempt()?)?.encode()?;
    for text in [
        "{}".to_owned(),
        " ".repeat(MAX_PROBE_BYTES + 1),
        encoded.replace("\"revision\":7", "\"revision\":0"),
        encoded.replace("\"revision\":7", "\"revision\":7,\"revision\":8"),
        encoded.replace(&"2".repeat(32), "../foreign"),
    ] {
        assert!(ProbeRequest::decode(text.as_bytes()).is_err());
    }
    // A field this reader does not know grants nothing: only the known correlation is kept and
    // the receiver still matches it against protected state.
    let foreign = encoded.replace("\"revision\":7", "\"revision\":7,\"repository\":\"foreign\"");
    assert_eq!(ProbeRequest::decode(foreign.as_bytes())?.attempt(), &attempt()?);
    let mut zero = attempt()?;
    zero.revision = 0;
    assert!(ProbeRequest::new(zero).is_err());
    Ok(())
}
