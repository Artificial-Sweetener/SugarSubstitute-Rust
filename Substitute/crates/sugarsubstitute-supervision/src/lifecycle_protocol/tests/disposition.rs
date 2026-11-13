// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve explicit execution policy through every grant without inferring native authority.
use super::*;
use sugarsubstitute_contracts::configuration::BackendDataDisposition;

#[test]
fn granted_and_reloaded_dispositions_have_closed_independent_golden_tags() -> TestResult {
    for (disposition, tag) in
        [(BackendDataDisposition::External, 1), (BackendDataDisposition::InstallationOwned, 2)]
    {
        let value =
            GrantedContext { backend_data_disposition: disposition, ..extensions::context()? };
        for reply in [ReplyOutcome::Granted(value), ReplyOutcome::Reloaded(value)] {
            let bytes = encoded(WirePayload::Reply(reply))?;
            assert_eq!(bytes.last(), Some(&tag));
            let actual = match FrameCodec::decode(&bytes)?.payload {
                DecodedPayload::Reply(ReplyOutcome::Granted(actual))
                | DecodedPayload::Reply(ReplyOutcome::Reloaded(actual)) => actual,
                _ => return Err("Expected exact grant context".into()),
            };
            assert_eq!(actual, value);
            assert_eq!(actual.backend_data_disposition(), disposition);
        }
    }
    Ok(())
}

#[test]
fn missing_and_unknown_grant_dispositions_never_default_to_owned_or_external() -> TestResult {
    let value = extensions::context()?;
    for reply in [ReplyOutcome::Granted(value), ReplyOutcome::Reloaded(value)] {
        let bytes = encoded(WirePayload::Reply(reply))?;
        for tag in [0, 3, u8::MAX] {
            let mut invalid = bytes.clone();
            *invalid.last_mut().ok_or("Missing disposition tag")? = tag;
            assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::UnsupportedTag)));
        }
        let mut missing = bytes[..bytes.len() - 1].to_vec();
        set_length(&mut missing);
        assert!(matches!(FrameCodec::decode(&missing), Err(ProtocolError::Truncated)));
    }
    Ok(())
}

#[test]
fn registration_disposition_has_exact_tag_and_rejects_unknown_or_missing_policy() -> TestResult {
    for (disposition, tag) in
        [(BackendDataDisposition::External, 1), (BackendDataDisposition::InstallationOwned, 2)]
    {
        let reply = ReplyOutcome::Registered { backend_data_disposition: disposition };
        let bytes = encoded(WirePayload::Reply(reply))?;
        assert_eq!(&bytes[HEADER_BYTES..], &[17, tag]);
        assert!(
            matches!(FrameCodec::decode(&bytes)?.payload, DecodedPayload::Reply(actual) if actual == reply)
        );
        for tag in [0, 3, u8::MAX] {
            let mut invalid = bytes.clone();
            invalid[HEADER_BYTES + 1] = tag;
            assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::UnsupportedTag)));
        }
        let mut missing = bytes[..bytes.len() - 1].to_vec();
        set_length(&mut missing);
        assert!(matches!(FrameCodec::decode(&missing), Err(ProtocolError::Truncated)));
    }
    Ok(())
}
