// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! RFC 6455 framing for the client side, without I/O.
//!
//! The decoder buffers at most one text message of `MAX_MESSAGE_BYTES`; longer messages and
//! every binary message are skipped as they stream in and surface as one `Discarded` frame.
use super::MAX_MESSAGE_BYTES;
use std::io;

/// Largest control payload the protocol permits.
const MAX_CONTROL_BYTES: usize = 125;

/// One complete unit the session acts on.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum Frame {
    /// A complete UTF-8 text message within the bound.
    Text(String),
    /// A message that was not kept: binary, or text over the bound. `bytes` is its full length.
    Discarded {
        /// Total payload bytes of the skipped message.
        bytes: u64,
        /// Whether the message was binary rather than oversized text.
        binary: bool,
    },
    /// A ping to answer with the same payload.
    Ping(Vec<u8>),
    /// The server's close, with its status code when it sent one.
    Close(Option<u16>),
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Kind {
    Text,
    Binary,
}

/// Header of the frame being read.
struct Header {
    fin: bool,
    opcode: u8,
    remaining: u64,
}

/// Incremental client-side decoder; feed it every byte the server sends, in order.
#[derive(Default)]
pub(super) struct Decoder {
    header: Vec<u8>,
    current: Option<Header>,
    control: Vec<u8>,
    message: Option<Kind>,
    text: Vec<u8>,
    discarding: bool,
    message_bytes: u64,
}

impl Decoder {
    /// Decode `input`, appending each completed frame to `out`.
    /// # Errors
    /// Rejects masked server frames, reserved bits and opcodes, fragmented or oversized control
    /// frames, continuation without a message, a new message inside another, and invalid UTF-8.
    pub(super) fn push(&mut self, mut input: &[u8], out: &mut Vec<Frame>) -> io::Result<()> {
        while !input.is_empty() {
            let Some(header) = &mut self.current else {
                self.header.push(input[0]);
                input = &input[1..];
                if let Some(header) = parse_header(&self.header)? {
                    self.header.clear();
                    self.begin(header, out)?;
                }
                continue;
            };
            let take = usize::try_from(header.remaining).unwrap_or(usize::MAX).min(input.len());
            let (chunk, rest) = input.split_at(take);
            input = rest;
            header.remaining -= take as u64;
            let (opcode, fin, done) = (header.opcode, header.fin, header.remaining == 0);
            if opcode >= 0x8 {
                self.control.extend_from_slice(chunk);
            } else {
                self.data(chunk);
            }
            if done {
                self.current = None;
                self.complete(opcode, fin, out)?;
            }
        }
        Ok(())
    }

    /// Start a frame; empty frames complete at once.
    fn begin(&mut self, header: Header, out: &mut Vec<Frame>) -> io::Result<()> {
        match header.opcode {
            0x0 if self.message.is_none() => {
                return Err(protocol("continuation without a message"));
            }
            0x1 | 0x2 if self.message.is_some() => {
                return Err(protocol("new message inside a fragmented message"));
            }
            0x1 => self.message = Some(Kind::Text),
            0x2 => self.message = Some(Kind::Binary),
            0x0 => {}
            0x8..=0xA => {
                if !header.fin || header.remaining > MAX_CONTROL_BYTES as u64 {
                    return Err(protocol("fragmented or oversized control frame"));
                }
                self.control.clear();
            }
            _ => return Err(protocol("reserved opcode")),
        }
        let (opcode, fin, empty) = (header.opcode, header.fin, header.remaining == 0);
        self.current = Some(header);
        if empty {
            self.current = None;
            self.complete(opcode, fin, out)?;
        }
        Ok(())
    }

    /// Account message payload, keeping only text within the bound.
    fn data(&mut self, chunk: &[u8]) {
        self.message_bytes = self.message_bytes.saturating_add(chunk.len() as u64);
        let keep = self.message == Some(Kind::Text)
            && !self.discarding
            && self.text.len() + chunk.len() <= MAX_MESSAGE_BYTES;
        if keep {
            self.text.extend_from_slice(chunk);
        } else {
            self.discarding = true;
            self.text = Vec::new();
        }
    }

    /// Finish a frame: a control frame acts at once; a data frame with FIN ends its message.
    fn complete(&mut self, opcode: u8, fin: bool, out: &mut Vec<Frame>) -> io::Result<()> {
        match opcode {
            0x8 => out.push(Frame::Close(close_code(&self.control)?)),
            0x9 => out.push(Frame::Ping(std::mem::take(&mut self.control))),
            0xA => self.control.clear(),
            _ if fin => {
                let kind = self.message.take().ok_or_else(|| protocol("message end absent"))?;
                let bytes = std::mem::take(&mut self.message_bytes);
                let text = std::mem::take(&mut self.text);
                let discarded = std::mem::take(&mut self.discarding);
                if kind == Kind::Binary || discarded {
                    out.push(Frame::Discarded { bytes, binary: kind == Kind::Binary });
                } else {
                    let text =
                        String::from_utf8(text).map_err(|_| protocol("text is not UTF-8"))?;
                    out.push(Frame::Text(text));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Parse a complete header once enough bytes arrived; `None` asks for more.
fn parse_header(bytes: &[u8]) -> io::Result<Option<Header>> {
    let [first, second, ..] = *bytes else { return Ok(None) };
    if first & 0x70 != 0 {
        return Err(protocol("reserved bits set"));
    }
    if second & 0x80 != 0 {
        return Err(protocol("server frame is masked"));
    }
    let (length_bytes, short) = match second & 0x7F {
        126 => (2, None),
        127 => (8, None),
        length => (0, Some(u64::from(length))),
    };
    if bytes.len() < 2 + length_bytes {
        return Ok(None);
    }
    let remaining = match short {
        Some(length) => length,
        None => bytes[2..2 + length_bytes]
            .iter()
            .fold(0_u64, |length, byte| (length << 8) | u64::from(*byte)),
    };
    if remaining >> 63 != 0 {
        return Err(protocol("payload length high bit set"));
    }
    Ok(Some(Header { fin: first & 0x80 != 0, opcode: first & 0x0F, remaining }))
}

/// Status code of a close payload: empty, or two bytes of code then UTF-8 reason.
fn close_code(payload: &[u8]) -> io::Result<Option<u16>> {
    match payload {
        [] => Ok(None),
        [_] => Err(protocol("one-byte close payload")),
        [high, low, reason @ ..] => {
            std::str::from_utf8(reason).map_err(|_| protocol("close reason is not UTF-8"))?;
            Ok(Some(u16::from_be_bytes([*high, *low])))
        }
    }
}

/// Encode one final client frame, masked with `mask` as the protocol requires.
pub(super) fn masked(opcode: u8, payload: &[u8], mask: [u8; 4]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(payload.len() + 14);
    frame.push(0x80 | opcode);
    match payload.len() {
        length @ 0..=125 => frame.push(0x80 | length as u8),
        length @ 126..=0xFFFF => {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(length as u16).to_be_bytes());
        }
        length => {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(length as u64).to_be_bytes());
        }
    }
    frame.extend_from_slice(&mask);
    frame.extend(payload.iter().enumerate().map(|(index, byte)| byte ^ mask[index % 4]));
    frame
}

fn protocol(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("WebSocket protocol violation: {reason}"))
}
