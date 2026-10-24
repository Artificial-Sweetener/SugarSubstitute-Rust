// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Frame a bounded UTF-8 worker stream without trusting chunk or Unicode boundaries.
use std::io;

#[derive(Default)]
pub(super) struct Decoder {
    pending: Vec<u8>,
}
impl Decoder {
    pub(super) fn push(&mut self, bytes: &[u8]) -> io::Result<Vec<String>> {
        let mut frames = Vec::new();
        for &byte in bytes {
            if self.pending.len() >= super::MAX_FRAME_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Worker frame exceeds bound",
                ));
            }
            if byte == b'\n' {
                let bytes = std::mem::take(&mut self.pending);
                frames.push(String::from_utf8(bytes).map_err(io::Error::other)?);
            } else {
                self.pending.push(byte);
            }
        }
        Ok(frames)
    }
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
    #[test]
    fn framing_preserves_split_unicode_and_rejects_partial_or_oversized_input() -> io::Result<()> {
        let mut decoder = Decoder::default();
        let mut frames = Vec::new();
        for byte in "\"作品\"\n{}\n".as_bytes() {
            frames.extend(decoder.push(&[*byte])?);
        }
        assert_eq!(frames, ["\"作品\"", "{}"]);
        decoder.finish()?;
        let mut partial = Decoder::default();
        partial.push(b"{")?;
        assert!(partial.finish().is_err());
        let mut large = Decoder::default();
        assert!(large.push(&vec![b'x'; super::super::MAX_FRAME_BYTES + 1]).is_err());
        Ok(())
    }
    #[test]
    fn command_overload_is_explicit_and_retains_the_queue_bound() {
        let (transport, _job) = super::super::Transport::pair();
        for _ in 0..super::super::QUEUE_SLOTS {
            assert!(transport.send("{}".into()).is_ok());
        }
        assert_eq!(
            transport.send("{}".into()).err().map(|error| error.kind()),
            Some(io::ErrorKind::WouldBlock)
        );
    }
}
