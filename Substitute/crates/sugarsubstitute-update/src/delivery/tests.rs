// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resume framing must never reinterpret server claims as signed target bounds.
use super::*;
use crate::{RepositoryTrust, fixture::*};
type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

fn target() -> Result<Target, Box<dyn std::error::Error + Send + Sync>> {
    let key = key(55)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    trust.refresh(Set::new(&key, [1; 3], b"1234567890", EXPIRES)?.metadata(), NOW)?;
    Ok(trust.target(NAME, NOW)?)
}
fn response(status: u16, length: Option<u64>, range: Option<&str>) -> Response<'_> {
    Response { status, length, range, encoding: None }
}

#[test]
fn full_response_restarts_partial_and_exact_tail_resumes() -> TestResult {
    let target = target()?;
    let full = Transfer::accept(&target, 4, response(200, Some(10), None))?;
    assert_eq!((full.offset(), full.length()), (0, 10));
    let tail = Transfer::accept(&target, 4, response(206, Some(6), Some("bytes 4-9/10")))?;
    assert_eq!((tail.offset(), tail.length()), (4, 6));
    assert!(Transfer::accept(&target, 4, response(206, None, Some("bytes 4-9/10"))).is_ok());
    let shorter = Transfer::accept(&target, 4, response(206, Some(2), Some("BYTES 4-5/10")))?;
    assert_eq!((shorter.offset(), shorter.length()), (4, 2));
    Ok(())
}

#[test]
fn conflicting_bounds_encoding_and_noncanonical_ranges_are_rejected() -> TestResult {
    let target = target()?;
    for range in [
        "bytes 0-9/10",
        "bytes 4-10/10",
        "bytes 4-9/11",
        "bytes 4-9/*",
        "bytes +4-9/10",
        "bytes 4-9/10, 0-1/10",
        "bytes 4-18446744073709551615/10",
        "bytes 9-4/10",
        "bytes 4-9/10 ",
    ] {
        assert!(
            Transfer::accept(&target, 4, response(206, Some(6), Some(range))).is_err(),
            "{range}"
        );
    }
    for value in [
        response(200, Some(6), None),
        response(200, Some(10), Some("bytes 0-9/10")),
        response(206, Some(6), None),
        Response { status: 200, length: Some(10), range: None, encoding: Some("gzip") },
    ] {
        assert!(Transfer::accept(&target, 4, value).is_err());
    }
    assert!(matches!(
        Transfer::accept(&target, 11, response(200, None, None)),
        Err(Rejection::Offset)
    ));
    Ok(())
}

#[test]
fn redirects_rate_limits_and_range_errors_are_not_body_authority() -> TestResult {
    let target = target()?;
    for status in [301, 302, 304, 307, 308, 401, 403, 404, 416, 429, 500, 503] {
        assert!(
            matches!(Transfer::accept(&target, 4, response(status, None, None)), Err(Rejection::Status(actual)) if actual == status)
        );
    }
    Ok(())
}
