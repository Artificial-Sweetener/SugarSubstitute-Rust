// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise closed wire shape against real domain validators and exact immutable bytes.
use super::*;
use std::{error::Error, path::PathBuf};
use sugarsubstitute_contracts::{
    configuration::{
        ConnectionConfiguration, LaunchConfiguration, LoopbackOrigin, Presentation, ProfileId,
        StartupConfiguration, Theme,
    },
    coordinator_entry::{EntryConnectionView, EntryOwnerDegradation},
    lifecycle::{CoordinatorKey, InstanceId, RunGeneration},
};

fn header() -> Result<Header, Box<dyn Error>> {
    Ok(Header {
        namespace: NamespaceId::new([1; 32]),
        epoch: Some(CoordinatorEpoch::new([2; 16])?),
        request: EntryRequestId::new(100)?,
        source: CounterDeadline::new(100, 10)?,
        nonce: [3; 32],
    })
}
fn path() -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(r"E:\entry-input")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/entry-input")
    }
}
fn request(
    connection: ConnectionConfiguration,
    package: PathBuf,
    environment: Vec<u16>,
) -> Result<EntryLaunchRequest, Box<dyn Error>> {
    let configuration = StartupConfiguration::new(
        connection,
        ProfileId::new("Profile_1".into())?,
        Presentation::new("en", Theme::Light, false)?,
    );
    Ok(EntryLaunchRequest::new(
        configuration,
        package,
        path(),
        path(),
        path(),
        path(),
        environment,
    )?)
}
fn attached() -> Result<ConnectionConfiguration, Box<dyn Error>> {
    Ok(ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://localhost:8188", 8188)?))
}

#[test]
fn all_closed_frames_preserve_correlation_and_exact_lengths() -> Result<(), Box<dyn Error>> {
    let bound = header()?;
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: bound.namespace,
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(7)?,
        generation: RunGeneration::new(9)?,
    };
    let mut unbound = bound;
    unbound.epoch = None;
    unbound.nonce = [0; 32];
    for (header, payload) in [
        (unbound, Payload::Hello),
        (unbound, Payload::Initializing),
        (bound, Payload::Challenge),
        (bound, Payload::Accepted(run)),
        (bound, Payload::Closing),
    ] {
        let mut output = [0u8; 256];
        let expected = encoded_length(&header, &payload)?;
        let length = encode(&mut output, &header, payload)?;
        assert_eq!(length, expected);
        let decoded = decode(&output[..length])?;
        assert!(decoded.header.same_source(header));
        match decoded.payload {
            DecodedPayload::Accepted(observed) => assert_eq!(observed, run),
            DecodedPayload::Hello
            | DecodedPayload::Initializing
            | DecodedPayload::Challenge
            | DecodedPayload::Closing => {}
            _ => return Err("unexpected closed payload".into()),
        }
        assert!(decode(&output[..length + 1]).is_err());
    }
    Ok(())
}
#[test]
fn every_typed_rejection_round_trips_including_stale_and_degradation() -> Result<(), Box<dyn Error>>
{
    let header = header()?;
    let reasons = [
        EntryRejection::InvalidInput,
        EntryRejection::ProfileBusy,
        EntryRejection::Capacity,
        EntryRejection::Deadline,
        EntryRejection::Authentication,
        EntryRejection::ChangedDuplicate,
        EntryRejection::RecoverableContention,
        EntryRejection::StaleRequest,
        EntryRejection::DegradedOwner(EntryOwnerDegradation::IdentityUncertain),
        EntryRejection::DegradedOwner(EntryOwnerDegradation::RecordUnknown),
        EntryRejection::DegradedOwner(EntryOwnerDegradation::EndpointCollision),
        EntryRejection::DegradedOwner(EntryOwnerDegradation::NativeCleanupUncertain),
    ];
    for reason in reasons {
        let mut output = [0; HEADER_BYTES + 2];
        let length = encode(&mut output, &header, Payload::Rejected(reason))?;
        match decode(&output[..length])?.payload {
            DecodedPayload::Rejected(observed) => assert_eq!(observed, reason),
            _ => return Err("expected closed rejection".into()),
        }
    }
    Ok(())
}
#[test]
fn managed_argv_environment_and_raw_paths_remain_exact() -> Result<(), Box<dyn Error>> {
    let arguments = vec!["".into(), "画像 café 🙂".into(), "$literal; & value".into()];
    let launch = LaunchConfiguration::new(path(), path(), arguments.clone(), 8188)?;
    let environment = vec![90, 61, 0xd800, 0, 65, 61, 50, 0, 61, 67, 58, 61, 67, 58, 92, 0, 0];
    #[cfg(windows)]
    let package = {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt};
        PathBuf::from(OsString::from_wide(&[69, 58, 92, 0xd800, 65, 0xdc01]))
    };
    #[cfg(not(windows))]
    let package = path();
    let request =
        request(ConnectionConfiguration::Launch(launch), package.clone(), environment.clone())?;
    let mut output = vec![0; MAX_ENTRY_FRAME_BYTES];
    let length = encode(&mut output, &header()?, Payload::Launch(&request))?;
    for _ in 0..2 {
        let DecodedPayload::Launch(view) = decode(&output[..length])?.payload else {
            return Err("expected launch".into());
        };
        let EntryConnectionView::Managed { arguments: observed, .. } = view.connection() else {
            return Err("expected managed".into());
        };
        assert_eq!(observed.iter().collect::<Vec<_>>(), arguments);
        assert_eq!(view.environment().units().collect::<Vec<_>>(), environment);
        assert_eq!(
            view.frontend().package_directory().units().collect::<Vec<_>>(),
            raw_windows::units(&package)?.collect::<Vec<_>>()
        );
        assert_eq!(view.profile(), "Profile_1");
    }
    Ok(())
}
#[test]
fn attachment_host_variants_use_exact_domain_origin() -> Result<(), Box<dyn Error>> {
    for origin in ["http://127.0.0.1:8188", "http://[::1]:8188", "http://localhost:8188"] {
        let origin = LoopbackOrigin::parse(origin, 8188)?;
        let request = request(ConnectionConfiguration::Attach(origin), path(), vec![0, 0])?;
        let mut output = vec![0; MAX_ENTRY_FRAME_BYTES];
        let length = encode(&mut output, &header()?, Payload::Launch(&request))?;
        let DecodedPayload::Launch(view) = decode(&output[..length])?.payload else {
            return Err("expected launch".into());
        };
        let EntryConnectionView::Attached(observed) = view.connection() else {
            return Err("expected attached".into());
        };
        assert_eq!(observed, origin);
    }
    Ok(())
}
#[test]
fn rejected_encoding_leaves_entire_output_untouched() -> Result<(), Box<dyn Error>> {
    let mut large = path();
    large.push("a".repeat(32700));
    let request = request(attached()?, large, vec![0, 0])?;
    let mut output = vec![0xa5; MAX_ENTRY_FRAME_BYTES];
    assert!(matches!(
        encode(&mut output, &header()?, Payload::Launch(&request)),
        Err(EntryTransportError::Capacity)
    ));
    assert!(output.iter().all(|byte| *byte == 0xa5));
    let mut short = [0xa5; HEADER_BYTES - 1];
    assert!(encode(&mut short, &header()?, Payload::Challenge).is_err());
    assert!(short.iter().all(|byte| *byte == 0xa5));
    Ok(())
}
#[test]
fn invalid_header_and_payload_fields_reject_closed_before_use() -> Result<(), Box<dyn Error>> {
    let mut original = [0u8; HEADER_BYTES + 2];
    encode(&mut original, &header()?, Payload::Rejected(EntryRejection::Capacity))?;
    for (range, value) in [
        (0..4, 0),
        (4..6, 2),
        (6..8, 255),
        (8..12, 255),
        (12..16, 1),
        (48..64, 0),
        (64..72, 0),
        (72..80, 0),
        (80..112, 0),
        (112..120, 0),
        (120..121, 255),
        (121..122, 255),
    ] {
        let mut corrupt = original;
        corrupt[range].fill(value);
        assert!(decode(&corrupt).is_err());
    }
    Ok(())
}
#[test]
fn retained_staging_changes_only_header_on_fresh_challenge() -> Result<(), Box<dyn Error>> {
    let request = request(attached()?, path(), vec![65, 61, 0xd800, 0, 0])?;
    let mut output = vec![0xa5; MAX_ENTRY_FRAME_BYTES];
    let length = stage_launch(&mut output, &request)?;
    assert!(output[..HEADER_BYTES].iter().all(|byte| *byte == 0));
    let body = output[HEADER_BYTES..length].to_vec();
    let mut bound = header()?;
    bind_staged_launch(&mut output, length, &bound)?;
    bound.nonce = [4; 32];
    bind_staged_launch(&mut output, length, &bound)?;
    assert_eq!(output[HEADER_BYTES..length], body);
    assert!(output[length..].iter().all(|byte| *byte == 0xa5));
    let decoded = decode(&output[..length])?;
    assert_eq!(decoded.header.nonce, [4; 32]);
    assert!(matches!(decoded.payload, DecodedPayload::Launch(_)));
    Ok(())
}
#[test]
fn exact_maximum_combined_frame_decodes_and_one_more_unit_preserves_output()
-> Result<(), Box<dyn Error>> {
    let baseline = request(attached()?, path(), vec![0, 0])?;
    let length = encoded_length(&header()?, &Payload::Launch(&baseline))?;
    assert_eq!((MAX_ENTRY_FRAME_BYTES - length) % 2, 0);
    let units = 2 + (MAX_ENTRY_FRAME_BYTES - length) / 2;
    let environment = |units: usize| {
        let mut block = Vec::with_capacity(units);
        block.extend([65, 61]);
        block.resize(units - 2, 97);
        block.extend([0, 0]);
        block
    };
    let exact = request(attached()?, path(), environment(units))?;
    let mut output = vec![0xa5; MAX_ENTRY_FRAME_BYTES];
    assert_eq!(encode(&mut output, &header()?, Payload::Launch(&exact))?, MAX_ENTRY_FRAME_BYTES);
    let DecodedPayload::Launch(view) = decode(&output)?.payload else {
        return Err("expected exact maximum launch".into());
    };
    assert_eq!(view.environment().units().count(), units);
    let excessive = request(attached()?, path(), environment(units + 1))?;
    output.fill(0xa5);
    assert!(matches!(stage_launch(&mut output, &excessive), Err(EntryTransportError::Capacity)));
    assert!(output.iter().all(|byte| *byte == 0xa5));
    Ok(())
}
