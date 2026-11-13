// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! How long each signed role stays valid, and when it is re-signed.
//!
//! The timestamp is re-signed daily, so it survives thirteen missed days. Snapshot and targets
//! are re-signed at every release and monthly, so they survive two missed months. The root is
//! rotated or re-signed yearly by its offline key holder; an installation's update check fails
//! closed once any role it needs has expired.
use crate::{Error, invalid};
use chrono::{DateTime, Utc};

const DAY: i64 = 24 * 60 * 60;
/// Timestamp lifetime: 14 days.
pub const TIMESTAMP_SECONDS: i64 = 14 * DAY;
/// Snapshot lifetime: 90 days.
pub const SNAPSHOT_SECONDS: i64 = 90 * DAY;
/// Targets lifetime: 90 days.
pub const TARGETS_SECONDS: i64 = 90 * DAY;
/// Root lifetime: one year.
pub const ROOT_SECONDS: i64 = 365 * DAY;

/// The expiry of a role signed at `now` (Unix UTC seconds) with a lifetime in seconds.
/// # Errors
/// Rejects negative times and times outside the representable calendar.
pub(crate) fn expiry(now: i64, lifetime: i64) -> Result<DateTime<Utc>, Error> {
    if now < 0 {
        return Err(invalid("Signing time must not precede 1970"));
    }
    now.checked_add(lifetime)
        .and_then(|seconds| DateTime::from_timestamp(seconds, 0))
        .ok_or_else(|| invalid("Signing time is outside the supported calendar"))
}
