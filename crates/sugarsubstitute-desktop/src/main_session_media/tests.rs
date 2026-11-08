// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound byte ranges and immutable browser replies independently of file availability.
use super::*;
use std::error::Error;
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryLease, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}
fn identity() -> Result<MainIdentity> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}
#[test]
fn native_single_byte_ranges_keep_exact_inclusive_open_and_suffix_meaning() -> Result {
    assert_eq!(
        SessionMediaRange::parse("bytes=0-9")?,
        SessionMediaRange::From { start: 0, end: Some(9) }
    );
    assert_eq!(
        SessionMediaRange::parse("bytes=23-")?,
        SessionMediaRange::From { start: 23, end: None }
    );
    assert_eq!(SessionMediaRange::parse("bytes=-64")?, SessionMediaRange::Suffix { bytes: 64 });
    for value in [
        "bytes=1-0",
        "bytes=-0",
        "bytes=0-1,3-4",
        "Bytes=0-1",
        "bytes= 0-1",
        "bytes=0-1 ",
        "bytes=+1-2",
        "bytes=18446744073709551616-",
        "bytes=",
        "bytes=1-2-3",
    ] {
        assert!(SessionMediaRange::parse(value).is_err(), "{value}");
    }
    Ok(())
}
#[test]
fn encoded_reply_transfers_actual_charge_and_refuses_unsafe_partial_totals() -> Result {
    let lease = MemoryLease::new(1024, Box::new(Release));
    let request = NonZeroU64::new(1).ok_or("Missing request")?;
    let reply =
        SessionMediaReply::new(identity()?, request, "image/gif", vec![1; 512], lease.claim(512)?)?;
    assert_eq!(reply.request_id(), request);
    assert!(lease.claim(513).is_err());
    drop(reply);
    assert!(lease.claim(1024).is_ok());
    assert!(
        SessionMediaReply::new(identity()?, request, "text/html", vec![1], lease.claim(1)?)
            .is_err()
    );
    assert!(
        SessionMediaReply::partial(
            identity()?,
            request,
            "video/mp4",
            vec![1; 10],
            lease.claim(10)?,
            95,
            100
        )
        .is_err()
    );
    assert!(
        SessionMediaReply::partial(
            identity()?,
            request,
            "video/webm",
            vec![1; 10],
            lease.claim(10)?,
            90,
            100
        )
        .is_ok()
    );
    assert!(SessionMediaReply::metadata(identity()?, request, "image/webp", 8192, true).is_ok());
    assert!(
        SessionMediaReply::metadata(
            identity()?,
            request,
            "video/mp4",
            sugarsubstitute_contracts::desktop_session::MAX_ASSET_BYTES + 1,
            false
        )
        .is_err()
    );
    Ok(())
}
