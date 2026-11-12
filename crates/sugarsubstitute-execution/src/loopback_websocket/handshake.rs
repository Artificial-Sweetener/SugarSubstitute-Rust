// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The RFC 6455 opening handshake for the client side, without I/O.
use sha1::{Digest, Sha1};
use std::io;

/// Largest response head accepted before the upgrade completes.
pub(super) const MAX_RESPONSE_HEAD: usize = 8192;
/// Fixed value appended to the client key before hashing, from RFC 6455 section 1.3.
const ACCEPT_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// The request bytes for `path` on `host`, offering `key`.
pub(super) fn request(host: &str, path: &str, key: &str) -> String {
    format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
    )
}

/// The base64 client key for 16 random bytes.
pub(super) fn key(random: &[u8; 16]) -> String {
    base64(random)
}

/// Length of the response head in `received`, through its blank line, once it is complete.
pub(super) fn head_length(received: &[u8]) -> Option<usize> {
    received.windows(4).position(|window| window == b"\r\n\r\n").map(|at| at + 4)
}

/// Check a complete response head against the offered `key`.
/// # Errors
/// Rejects a status other than 101, missing upgrade headers, and a wrong accept value.
pub(super) fn verify(head: &[u8], key: &str) -> io::Result<()> {
    let head = std::str::from_utf8(head).map_err(|_| refused("response head is not UTF-8"))?;
    let mut lines = head.split("\r\n");
    let status = lines.next().unwrap_or_default();
    let mut parts = status.splitn(3, ' ');
    if parts.next() != Some("HTTP/1.1") || parts.next() != Some("101") {
        return Err(refused("server did not switch protocols"));
    }
    let (mut upgrade, mut connection, mut accept) = (false, false, None);
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').ok_or_else(|| refused("malformed header"))?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("upgrade") {
            upgrade = value.eq_ignore_ascii_case("websocket");
        } else if name.eq_ignore_ascii_case("connection") {
            connection = value.split(',').any(|token| token.trim().eq_ignore_ascii_case("upgrade"));
        } else if name.eq_ignore_ascii_case("sec-websocket-accept") {
            accept = Some(value);
        }
    }
    if !upgrade || !connection {
        return Err(refused("upgrade headers absent"));
    }
    if accept != Some(expected_accept(key).as_str()) {
        return Err(refused("accept value does not match the key"));
    }
    Ok(())
}

/// The accept value a conforming server derives from `key`.
pub(super) fn expected_accept(key: &str) -> String {
    let mut hash = Sha1::new();
    hash.update(key.as_bytes());
    hash.update(ACCEPT_GUID.as_bytes());
    base64(&hash.finalize())
}

/// Standard padded base64; the handshake encodes only 16- and 20-byte values.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let group = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |group, (index, byte)| group | (u32::from(*byte) << (16 - 8 * index)));
        for index in 0..4 {
            if index <= chunk.len() {
                encoded.push(char::from(ALPHABET[(group >> (18 - 6 * index)) as usize & 63]));
            } else {
                encoded.push('=');
            }
        }
    }
    encoded
}

fn refused(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::ConnectionRefused, format!("WebSocket upgrade refused: {reason}"))
}
