// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve exact frame/body bytes and whole-body preflight at the sole codec.
use super::*;
#[test]
fn startup_launch_body_matches_original_full_and_staged_frames() -> Result<(), Box<dyn Error>> {
    for connection in [
        attached()?,
        ConnectionConfiguration::Launch(LaunchConfiguration::new(
            path(),
            path(),
            vec!["".into(), "a \"b\" c".into()],
            8188,
        )?),
    ] {
        let original = request(connection, path(), vec![65, 61, 0xd800, 0, 0])?;
        let body_length = launch_payload::encoded_length(&original)?;
        let mut body = vec![0xa5; body_length + 1];
        assert_eq!(launch_payload::encode(&mut body, &original)?, body_length);
        assert_eq!(body[body_length], 0xa5);
        let mut frame = vec![0xa5; MAX_ENTRY_FRAME_BYTES];
        let length = encode(&mut frame, &header()?, Payload::Launch(&original))?;
        assert_eq!(body[..body_length], frame[HEADER_BYTES..length]);
        frame.fill(0xa5);
        assert_eq!(stage_launch(&mut frame, &original)?, length);
        assert_eq!(body[..body_length], frame[HEADER_BYTES..length]);
        let view = launch_payload::decode(&body[..body_length])?;
        assert_eq!(view.environment().units().collect::<Vec<_>>(), vec![65, 61, 0xd800, 0, 0]);
        assert_eq!(view.profile(), "Profile_1");
    }
    Ok(())
}
#[test]
fn startup_launch_body_rejects_empty_truncated_trailing_and_reserved_before_use()
-> Result<(), Box<dyn Error>> {
    let original = request(attached()?, path(), vec![0, 0])?;
    let length = launch_payload::encoded_length(&original)?;
    let mut bytes = vec![0; length];
    launch_payload::encode(&mut bytes, &original)?;
    for end in 0..length {
        assert!(launch_payload::decode(&bytes[..end]).is_err());
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(launch_payload::decode(&extra).is_err());
    let mut reserved = bytes.clone();
    reserved[8] = 1;
    assert!(launch_payload::decode(&reserved).is_err());
    let mut short = vec![0xa5; length - 1];
    assert!(matches!(
        launch_payload::encode(&mut short, &original),
        Err(EntryTransportError::Capacity)
    ));
    assert!(short.iter().all(|byte| *byte == 0xa5));
    Ok(())
}
#[test]
fn startup_launch_body_maximum_is_the_unchanged_frame_minus_header_bound()
-> Result<(), Box<dyn Error>> {
    let initial = request(attached()?, path(), vec![0, 0])?;
    let baseline = launch_payload::encoded_length(&initial)?;
    let maximum = MAX_ENTRY_FRAME_BYTES - HEADER_BYTES;
    assert_eq!((maximum - baseline) % 2, 0);
    let units = 2 + (maximum - baseline) / 2;
    let environment = |units: usize| {
        let mut bytes = Vec::with_capacity(units);
        bytes.extend([65, 61]);
        bytes.resize(units - 2, 97);
        bytes.extend([0, 0]);
        bytes
    };
    let exact = request(attached()?, path(), environment(units))?;
    let mut bytes = vec![0xa5; maximum];
    assert_eq!(launch_payload::encoded_length(&exact)?, maximum);
    assert_eq!(launch_payload::encode(&mut bytes, &exact)?, maximum);
    assert_eq!(launch_payload::decode(&bytes)?.environment().unit_len(), units);
    let excessive = request(attached()?, path(), environment(units + 1))?;
    bytes.fill(0xa5);
    assert!(matches!(
        launch_payload::encode(&mut bytes, &excessive),
        Err(EntryTransportError::Capacity)
    ));
    assert!(bytes.iter().all(|byte| *byte == 0xa5));
    assert!(launch_payload::decode(&vec![0; maximum + 1]).is_err());
    Ok(())
}
