// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject ambiguous positive main identity and bounded stage inputs before encoding effects.
use super::extensions::{context, failure};
use super::*;
use std::num::NonZeroU32;

#[test]
fn controller_and_browser_authorization_require_positive_exact_pid_shapes() -> TestResult {
    let identity = main()?;
    let pid = NonZeroU32::new(u32::MAX).ok_or("zero PID")?;
    for (payload, offset) in [
        (WirePayload::ControllerPrepared { identity, browser_process_id: pid }, 120),
        (
            WirePayload::Reply(ReplyOutcome::BrowserAuthorized {
                identity,
                browser_process_id: pid,
            }),
            121,
        ),
    ] {
        let mut source = encoded(payload)?;
        assert_eq!(&source[offset..offset + 4], &u32::MAX.to_le_bytes());
        source[offset..offset + 4].fill(0);
        assert!(matches!(FrameCodec::decode(&source), Err(ProtocolError::Identity)));
    }
    Ok(())
}

#[test]
fn invalid_folder_progress_cannot_mutate_encoded_storage_or_be_decoded() -> TestResult {
    let identity = main()?;
    for value in [0, 32767, u32::MAX] {
        let payload = WirePayload::Reply(ReplyOutcome::FolderChunkAccepted {
            identity,
            next_offset_units: value,
        });
        let mut output = [0x72; 128];
        assert!(matches!(
            FrameCodec::encode(&WireMessage { header: header(11)?, payload }, &mut output),
            Err(ProtocolError::FragmentShape)
        ));
        assert_eq!(output, [0x72; 128]);
        let mut source = encoded(WirePayload::Reply(ReplyOutcome::FolderChunkAccepted {
            identity,
            next_offset_units: 1,
        }))?;
        source[121..125].copy_from_slice(&value.to_le_bytes());
        assert!(matches!(FrameCodec::decode(&source), Err(ProtocolError::FragmentShape)));
    }
    Ok(())
}
#[test]
fn payload_header_full_run_mismatch_leaves_storage_unchanged() -> TestResult {
    let mut foreign = main()?;
    foreign.run.owner.namespace = NamespaceId::new([0x99; 32]);
    let target = BackendTarget::Attached(context()?.origin);
    let context = GrantedContext { identity: foreign, ..context()? };
    let fragment = FolderFragment::new(foreign, 1, 0, RawWindowsUnits::from_units(&[69])?)?;
    for payload in [
        WirePayload::Acquire { identity: foreign, target: &target },
        WirePayload::Reload { lease: lease()?, identity: foreign },
        WirePayload::RequestedProfileFolder(fragment),
        WirePayload::EnvironmentPrepared(fragment),
        WirePayload::MainRetired(foreign),
        WirePayload::ControllerPrepared {
            identity: foreign,
            browser_process_id: NonZeroU32::new(1).ok_or("zero PID")?,
        },
        WirePayload::Reply(ReplyOutcome::Granted(context)),
        WirePayload::Reply(ReplyOutcome::EnvironmentAuthorized(foreign)),
        WirePayload::Reply(ReplyOutcome::BrowserAuthorized {
            identity: foreign,
            browser_process_id: NonZeroU32::new(1).ok_or("zero PID")?,
        }),
        WirePayload::Reply(ReplyOutcome::FolderChunkAccepted {
            identity: foreign,
            next_offset_units: 1,
        }),
        WirePayload::MainFailed(MainStageFailure { identity: foreign, ..failure()? }),
        WirePayload::Reply(ReplyOutcome::MainStageFailed(MainStageFailure {
            identity: foreign,
            ..failure()?
        })),
    ] {
        let message = WireMessage { header: header(payload.kind())?, payload };
        let mut output = vec![0x72; MAX_FRAME_BYTES];
        assert!(matches!(FrameCodec::encode(&message, &mut output), Err(ProtocolError::Identity)));
        assert!(output.iter().all(|byte| *byte == 0x72));
    }
    Ok(())
}

#[test]
fn wire_versions_and_stage_discriminators_and_zero_main_facts_reject() -> TestResult {
    let source = encoded(WirePayload::MainFailed(failure()?))?;
    for version in [1u16, 3, u16::MAX] {
        let mut invalid = source.clone();
        invalid[4..6].copy_from_slice(&version.to_le_bytes());
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Schema)));
    }
    for offset in [104, 112] {
        let mut invalid = source.clone();
        invalid[offset..offset + 8].fill(0);
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Identity)));
    }
    for (offset, value) in [(120, 0), (120, 6), (121, 0), (121, 7)] {
        let mut invalid = source.clone();
        invalid[offset] = value;
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::UnsupportedTag)));
    }
    Ok(())
}
