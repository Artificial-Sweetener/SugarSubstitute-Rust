// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve Unicode/argv spelling and preflight all budgets before domain allocation.
use super::*;

fn raw_managed(arguments: &[&str]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut source = encoded(WirePayload::Register)?;
    source[12..14].copy_from_slice(&4u16.to_le_bytes());
    source.extend(37u64.to_le_bytes());
    source.extend(41u64.to_le_bytes());
    source.push(1);
    source.extend(8188u16.to_le_bytes());
    source.push(arguments.len() as u8);
    for value in ["E:\\r", "E:\\p"].into_iter().chain(arguments.iter().copied()) {
        source.extend((value.encode_utf16().count() as u32).to_le_bytes());
        for unit in value.encode_utf16() {
            source.extend(unit.to_le_bytes());
        }
    }
    set_length(&mut source);
    Ok(source)
}
#[test]
fn managed_unicode_empty_and_shell_arguments_roundtrip_exactly_without_exposure() -> TestResult {
    let target = managed(vec![
        String::new(),
        "quoted\" trailing\\".into(),
        "a&b|c$(d);😀é".into(),
        "e\u{301}".into(),
    ])?;
    let source = encoded(WirePayload::Acquire { identity: main()?, target: (&target).into() })?;
    let decoded = FrameCodec::decode(&source)?;
    let DecodedPayload::Acquire { target: reference, .. } = decoded.payload else {
        return Err("Expected managed target".into());
    };
    assert_eq!(reference.to_domain()?, target);
    let DecodedTarget::Managed(reference) = reference else {
        return Err("Expected managed view".into());
    };
    assert_eq!(reference.arguments().len(), 4);
    assert_eq!(reference.arguments().iter().next().ok_or("Missing empty argument")??.unit_len(), 0);
    assert_eq!(reference.port().get(), 8188);
    let formatted = format!("{decoded:?}");
    for sensitive in ["quoted", "trailing", "a&b", "python.exe", "甜", "😀"] {
        assert!(!formatted.contains(sensitive));
    }
    Ok(())
}
#[test]
fn every_supported_attachment_host_and_port_roundtrips_without_arbitrary_url_data() -> TestResult {
    for host in ["127.0.0.1", "[::1]", "localhost"] {
        let target =
            BackendTarget::Attached(LoopbackOrigin::parse(&format!("http://{host}:65535"), 65535)?);
        let source = encoded(WirePayload::Acquire { identity: main()?, target: (&target).into() })?;
        assert_eq!(source.len(), 124);
        let DecodedPayload::Acquire { target: decoded, .. } = FrameCodec::decode(&source)?.payload
        else {
            return Err("Expected attachment".into());
        };
        assert_eq!(decoded.to_domain()?, target);
        for offset in [120, 121] {
            let mut invalid = source.clone();
            invalid[offset] = 0;
            assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::UnsupportedTag)));
        }
        let mut invalid = source;
        invalid[122..124].fill(0);
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Configuration(_))));
    }
    Ok(())
}
#[test]
fn exact_64k_frame_encodes_and_one_additional_unit_rejects_without_output_changes() -> TestResult {
    let mut arguments = vec!["x".repeat(4096); 7];
    arguments.push("y".repeat(4006));
    let target = BackendTarget::Managed(LaunchConfiguration::new(
        PathBuf::from("E:\\r"),
        PathBuf::from("E:\\p"),
        arguments.clone(),
        8188,
    )?);
    let source = encoded(WirePayload::Acquire { identity: main()?, target: (&target).into() })?;
    assert_eq!(source.len(), MAX_FRAME_BYTES);
    let DecodedPayload::Acquire { target: decoded, .. } = FrameCodec::decode(&source)?.payload
    else {
        return Err("Expected managed target".into());
    };
    assert_eq!(decoded.to_domain()?, target);
    arguments[7].push('z');
    let oversized = BackendTarget::Managed(LaunchConfiguration::new(
        PathBuf::from("E:\\r"),
        PathBuf::from("E:\\p"),
        arguments,
        8188,
    )?);
    let message = WireMessage {
        header: header(4)?,
        payload: WirePayload::Acquire { identity: main()?, target: (&oversized).into() },
    };
    let mut output = vec![0x72; MAX_FRAME_BYTES];
    assert!(matches!(FrameCodec::encode(&message, &mut output), Err(ProtocolError::FrameTooLarge)));
    assert!(output.iter().all(|byte| *byte == 0x72));
    Ok(())
}
#[test]
fn malformed_utf16_nul_and_counted_lengths_reject_before_owned_conversion() -> TestResult {
    let source = raw_managed(&["test"])?;
    for unit in [0u16, 0xD800, 0xDC00] {
        let mut invalid = source.clone();
        invalid[128..130].copy_from_slice(&unit.to_le_bytes());
        assert!(matches!(
            FrameCodec::decode(&invalid),
            Err(ProtocolError::InteriorNul | ProtocolError::InvalidUtf16)
        ));
    }
    for units in [32767u32, u32::MAX] {
        let mut invalid = source.clone();
        invalid[124..128].copy_from_slice(&units.to_le_bytes());
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Length)));
    }
    let mut invalid = source.clone();
    invalid[123] = 65;
    assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Length)));
    let mut invalid = source;
    invalid[121..123].fill(0);
    assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Configuration(_))));
    Ok(())
}
#[test]
fn complete_utf8_argument_budgets_are_checked_while_payloads_still_borrowed() -> TestResult {
    let maximum = "é".repeat(2048);
    let source = raw_managed(&[&maximum])?;
    assert!(FrameCodec::decode(&source).is_ok());
    let invalid = "é".repeat(2049);
    assert!(matches!(FrameCodec::decode(&raw_managed(&[&invalid])?), Err(ProtocolError::Length)));
    let euro = "€".repeat(1000);
    let too_many_bytes = raw_managed(&vec![euro.as_str(); 22])?;
    assert!(too_many_bytes.len() < MAX_FRAME_BYTES);
    assert!(matches!(FrameCodec::decode(&too_many_bytes), Err(ProtocolError::Length)));
    let source = raw_managed(&vec![""; 64])?;
    let DecodedPayload::Acquire { target: DecodedTarget::Managed(target), .. } =
        FrameCodec::decode(&source)?.payload
    else {
        return Err("Expected managed view".into());
    };
    assert_eq!(target.arguments().len(), 64);
    assert!(
        target.arguments().iter().all(|argument| argument.is_ok_and(|value| value.unit_len() == 0))
    );
    Ok(())
}
#[test]
fn explicit_domain_conversion_rejects_relative_and_dot_paths_without_normalizing() -> TestResult {
    for root in ["foo!", "E:.."] {
        let mut source = raw_managed(&[])?;
        for (bytes, unit) in source[128..136].chunks_exact_mut(2).zip(root.encode_utf16()) {
            bytes.copy_from_slice(&unit.to_le_bytes());
        }
        let DecodedPayload::Acquire { target, .. } = FrameCodec::decode(&source)?.payload else {
            return Err("Expected target".into());
        };
        assert!(matches!(target.to_domain(), Err(ProtocolError::Configuration(_))));
    }
    Ok(())
}

#[test]
fn configured_managed_view_preserves_exact_existing_acquire_bytes() -> Result<(), Box<dyn Error>> {
    let target = managed(vec!["--name=Unicode-雪".into(), "".into()])?;
    let BackendTarget::Managed(configuration) = &target else {
        return Err("Managed fixture changed".into());
    };
    let owned_bytes =
        encoded(WirePayload::Acquire { identity: main()?, target: (&target).into() })?;
    let borrowed = BorrowedBackendTarget::Managed(configuration);
    let borrowed_bytes = encoded(WirePayload::Acquire { identity: main()?, target: borrowed })?;
    assert_eq!(owned_bytes, borrowed_bytes);
    let BorrowedBackendTarget::Managed(retained) = borrowed else {
        return Err("Borrowed managed view changed".into());
    };
    assert!(std::ptr::eq(configuration, retained));
    Ok(())
}

#[test]
fn configured_attachment_view_preserves_exact_existing_acquire_bytes() -> Result<(), Box<dyn Error>>
{
    for host in ["127.0.0.1", "[::1]", "localhost"] {
        let origin = LoopbackOrigin::parse(&format!("http://{host}:8188"), 8188)?;
        let target = BackendTarget::Attached(origin);
        let configuration =
            sugarsubstitute_contracts::configuration::ConnectionConfiguration::Attach(origin);
        assert_eq!(
            encoded(WirePayload::Acquire { identity: main()?, target: (&target).into() })?,
            encoded(WirePayload::Acquire { identity: main()?, target: (&configuration).into() })?
        );
    }
    Ok(())
}
