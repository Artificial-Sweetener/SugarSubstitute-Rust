// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Frame a bounded UTF-8 worker stream without trusting chunk or Unicode boundaries.
use std::io;

/// Classify only the supported event tag, skipping payload syntax without building a value tree.
pub(super) fn is_download(frame: &str) -> io::Result<bool> {
    /// Inspect routing without constructing a second full message tree.
    #[derive(serde::Deserialize)]
    struct Header {
        event: String,
    }
    let header: Header = serde_json::from_str(frame).map_err(io::Error::other)?;
    Ok(header.event == "downloads")
}

/// Parse only authentication fields; unknown fields never build an arbitrary JSON value tree.
pub(super) fn authenticate(frame: &str, token: &str) -> io::Result<()> {
    /// Supported credential envelope; unrelated fields are skipped by the deserializer.
    #[derive(serde::Deserialize)]
    struct Authentication {
        version: u64,
        event: String,
        token: String,
    }
    let authentication: Authentication = serde_json::from_str(frame)
        .map_err(|cause| io::Error::new(io::ErrorKind::PermissionDenied, cause))?;
    if authentication.version != 1
        || authentication.event != "authenticate"
        || authentication.token != token
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Worker authentication failed",
        ));
    }
    Ok(())
}

/// Retain only the current incomplete message, never a batch of decoded allocations.
#[derive(Default)]
pub(super) struct Decoder {
    pending: Vec<u8>,
}
impl Decoder {
    /// Consume one byte and transfer at most one complete allocation to the caller.
    pub(super) fn push(&mut self, byte: u8) -> io::Result<Option<String>> {
        if self.pending.len() >= super::MAX_FRAME_BYTES {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Worker frame exceeds bound"));
        }
        if byte == b'\n' {
            let bytes = std::mem::take(&mut self.pending);
            return Ok(Some(String::from_utf8(bytes).map_err(io::Error::other)?));
        }
        self.pending.push(byte);
        Ok(None)
    }
    /// Reject an unterminated trailing frame when the peer actually closes.
    pub(super) fn finish(self) -> io::Result<()> {
        if self.pending.is_empty() {
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Truncated worker frame"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Preserve supported unknown fields and escaped credentials without allocating their JSON tree.
    #[test]
    fn authentication_checks_required_fields_and_skips_unknown_structure() -> io::Result<()> {
        authenticate(
            r#"{"version":1,"event":"authenticate","token":"\u0061","extra":[{},[],null]}"#,
            "a",
        )?;
        for frame in [
            r#"{"version":2,"event":"authenticate","token":"a"}"#,
            r#"{"version":1,"event":"authenticate","token":"wrong"}"#,
            r#"{"version":1,"event":"other","token":"a"}"#,
            r#"{"version":1,"event":"authenticate","token":{}}"#,
            "[]",
        ] {
            assert_eq!(
                authenticate(frame, "a").err().map(|cause| cause.kind()),
                Some(io::ErrorKind::PermissionDenied)
            );
        }
        Ok(())
    }

    /// The inclusive wire bound counts the delimiter and UTF-8 is checked only for complete frames.
    #[test]
    fn exact_maximum_delimited_frame_is_accepted_and_invalid_utf8_is_rejected() -> io::Result<()> {
        let mut decoder = Decoder::default();
        for _ in 0..super::super::MAX_FRAME_BYTES - 1 {
            assert!(decoder.push(b'x')?.is_none());
        }
        let frame = decoder.push(b'\n')?.ok_or_else(|| io::Error::other("Frame missing"))?;
        assert_eq!(frame.len(), super::super::MAX_FRAME_BYTES - 1);
        assert!(frame.capacity() <= super::super::MAX_FRAME_BYTES);
        let mut invalid = Decoder::default();
        assert!(invalid.push(0xff)?.is_none());
        assert!(invalid.push(b'\n').is_err());
        Ok(())
    }

    /// Split socket reads preserve Unicode and reject unterminated or overlong messages.
    #[test]
    fn framing_preserves_split_unicode_and_rejects_partial_or_oversized_input() -> io::Result<()> {
        let mut decoder = Decoder::default();
        let mut frames = Vec::new();
        for &byte in "\"作品\"\n{}\n".as_bytes() {
            frames.extend(decoder.push(byte)?);
        }
        assert_eq!(frames, ["\"作品\"", "{}"]);
        decoder.finish()?;
        let mut partial = Decoder::default();
        partial.push(b'{')?;
        assert!(partial.finish().is_err());
        let mut large = Decoder::default();
        for _ in 0..super::super::MAX_FRAME_BYTES {
            assert!(large.push(b'x')?.is_none());
        }
        assert!(large.push(b'x').is_err());
        Ok(())
    }
    /// Admission does not permit a seventeenth queued command.
    #[test]
    fn command_overload_is_explicit_and_retains_the_queue_bound() {
        let (transport, _job) = super::super::Transport::pair();
        let memory = sugarsubstitute_contracts::execution::MemoryLease::new(
            super::super::RESERVATION_BYTES,
            Box::new(super::super::tests::Release::default()),
        );
        assert!(transport.budget.activate(&memory).is_ok());
        for _ in 0..super::super::QUEUE_SLOTS {
            assert!(transport.send("{}".into()).is_ok());
        }
        assert_eq!(
            transport.send("{}".into()).err().map(|error| error.kind()),
            Some(io::ErrorKind::WouldBlock)
        );
    }
}
