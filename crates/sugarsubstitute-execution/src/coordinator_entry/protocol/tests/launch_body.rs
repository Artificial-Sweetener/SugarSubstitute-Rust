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
    // The one-byte optional-reference tag changes parity. A legitimate profile spelling
    // supplies the other byte; the exact frame/body budgets and UTF-16 rules stay unchanged.
    let requested = |environment| -> Result<EntryLaunchRequest, Box<dyn Error>> {
        request_with_profile(attached()?, path(), environment, "Profile_12")
    };
    let initial = requested(vec![0, 0])?;
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
    let exact = requested(environment(units))?;
    let mut bytes = vec![0xa5; maximum];
    assert_eq!(launch_payload::encoded_length(&exact)?, maximum);
    assert_eq!(launch_payload::encode(&mut bytes, &exact)?, maximum);
    assert_eq!(launch_payload::decode(&bytes)?.environment().unit_len(), units);
    assert_eq!(launch_payload::profile_length(&bytes, &path())?, maximum);
    let mut projected = vec![0xa5; maximum];
    assert!(
        launch_payload::project_profile(&mut projected, &bytes, &path().join("larger")).is_err()
    );
    assert!(projected.iter().all(|byte| *byte == 0xa5));
    let excessive = requested(environment(units + 1))?;
    bytes.fill(0xa5);
    assert!(matches!(
        launch_payload::encode(&mut bytes, &excessive),
        Err(EntryTransportError::Capacity)
    ));
    assert!(bytes.iter().all(|byte| *byte == 0xa5));
    assert!(launch_payload::decode(&vec![0; maximum + 1]).is_err());
    Ok(())
}
#[test]
fn startup_launch_body_carries_each_canvas_renderer_preference_exactly()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::{
        configuration::CanvasRendererPreference, coordinator_entry::EntryCanvasRendererView,
    };
    for (preference, expected) in [
        (CanvasRendererPreference::Absent, EntryCanvasRendererView::Absent),
        (
            CanvasRendererPreference::stored("vulkan:10de:2b85"),
            EntryCanvasRendererView::Stored("vulkan:10de:2b85"),
        ),
        (CanvasRendererPreference::Malformed, EntryCanvasRendererView::Malformed),
    ] {
        let configuration = StartupConfiguration::new(
            attached()?,
            ProfileId::new("Profile_1".into())?,
            Presentation::new("en", Theme::Light, false)?,
        )
        .with_canvas_renderer(preference.clone());
        let original = EntryLaunchRequest::new(
            configuration,
            path(),
            path(),
            path(),
            path(),
            path(),
            vec![0, 0],
        )?;
        let length = launch_payload::encoded_length(&original)?;
        let mut body = vec![0; length];
        assert_eq!(launch_payload::encode(&mut body, &original)?, length);
        let view = launch_payload::decode(&body)?;
        assert_eq!(view.canvas_renderer(), expected);
        assert_eq!(view.canvas_renderer().to_owned_preference(), preference);
        let mut unknown = body.clone();
        let kind = length
            - 3
            - match preference {
                CanvasRendererPreference::Stored(text) => text.len(),
                _ => 0,
            };
        unknown[kind] = 3;
        assert!(launch_payload::decode(&unknown).is_err());
    }
    Ok(())
}

#[test]
fn originals_reference_survives_full_staged_body_and_header_rebinding_without_dynamic_files()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::coordinator_entry::EntryOriginalsReference;
    let reference = EntryOriginalsReference::new([7; 32])?;
    let original = request(attached()?, path(), vec![0, 0])?.with_originals(reference);
    let body_length = launch_payload::encoded_length(&original)?;
    let mut body = vec![0; body_length];
    launch_payload::encode(&mut body, &original)?;
    assert_eq!(launch_payload::decode(&body)?.originals(), Some(reference));
    assert_eq!(&body[body.len() - 33..], &[&[1][..], &[7; 32][..]].concat());
    let mut full = vec![0; MAX_ENTRY_FRAME_BYTES];
    let length = encode(&mut full, &header()?, Payload::Launch(&original))?;
    assert_eq!(body, full[HEADER_BYTES..length]);
    let mut staged = vec![0; MAX_ENTRY_FRAME_BYTES];
    assert_eq!(stage_launch(&mut staged, &original)?, length);
    bind_staged_launch(&mut staged, length, &header()?)?;
    assert_eq!(staged[..length], full[..length]);
    assert_eq!(u16::from_le_bytes([full[4], full[5]]), 3);
    full[4..6].copy_from_slice(&2_u16.to_le_bytes());
    assert!(decode(&full[..length]).is_err());
    for kind in [2, 255] {
        let mut unknown = body.clone();
        unknown[body.len() - 33] = kind;
        assert!(launch_payload::decode(&unknown).is_err());
    }
    let mut zero = body.clone();
    zero[body.len() - 32..].fill(0);
    assert!(launch_payload::decode(&zero).is_err());
    for end in body.len() - 32..body.len() {
        assert!(launch_payload::decode(&body[..end]).is_err());
    }
    Ok(())
}

#[test]
fn selected_child_profile_preserves_original_reference_and_every_other_encoded_byte()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::coordinator_entry::EntryOriginalsReference;
    let original = request(attached()?, path(), vec![65, 61, 0xd800, 0, 0])?
        .with_originals(EntryOriginalsReference::new([7; 32])?);
    let mut bytes = vec![0; launch_payload::encoded_length(&original)?];
    launch_payload::encode(&mut bytes, &original)?;
    let retained = bytes.clone();
    let selected = path().join("user").join("desktop-data").join("profile");
    let length = launch_payload::profile_length(&bytes, &selected)?;
    let mut projected = vec![0xa5; length + 1];
    assert_eq!(launch_payload::project_profile(&mut projected, &bytes, &selected)?, length);
    assert_eq!(projected[length], 0xa5);
    let mut parts = original.into_parts();
    parts.profile_directory = selected;
    let expected = EntryLaunchRequest::new(
        parts.configuration,
        parts.package,
        parts.metadata,
        parts.cube,
        parts.script,
        parts.profile_directory,
        parts.environment,
    )?
    .with_originals(parts.originals.ok_or("Original reference lost")?);
    let mut encoded = vec![0; launch_payload::encoded_length(&expected)?];
    launch_payload::encode(&mut encoded, &expected)?;
    assert_eq!(projected[..length], encoded);
    assert_eq!(bytes, retained);
    assert_eq!(
        launch_payload::decode(&bytes)?.originals(),
        launch_payload::decode(&encoded)?.originals()
    );
    Ok(())
}
#[test]
fn selected_child_profile_rejects_malformed_original_growth_path_and_capacity_before_output()
-> Result<(), Box<dyn Error>> {
    let original = request(attached()?, path(), vec![0, 0])?;
    let mut bytes = vec![0; launch_payload::encoded_length(&original)?];
    launch_payload::encode(&mut bytes, &original)?;
    let profile = path().join("selected");
    let length = launch_payload::profile_length(&bytes, &profile)?;
    let mut output = vec![0xa5; length - 1];
    assert!(launch_payload::project_profile(&mut output, &bytes, &profile).is_err());
    assert!(output.iter().all(|byte| *byte == 0xa5));
    let mut output = vec![0xa5; MAX_ENTRY_FRAME_BYTES];
    for source in [&[][..], &bytes[..bytes.len() - 1]] {
        assert!(launch_payload::project_profile(&mut output, source, &profile).is_err());
        assert!(output.iter().all(|byte| *byte == 0xa5));
    }
    for profile in
        [std::path::PathBuf::from("relative"), path().join("a".repeat(MAX_ENTRY_FRAME_BYTES))]
    {
        assert!(launch_payload::project_profile(&mut output, &bytes, &profile).is_err());
        assert!(output.iter().all(|byte| *byte == 0xa5));
    }
    Ok(())
}
