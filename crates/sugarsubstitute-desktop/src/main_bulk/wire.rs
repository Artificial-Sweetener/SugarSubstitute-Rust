// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Spell bulk tokens, envelopes and origin routes exactly; every other spelling is rejected.

use super::MAX_MESSAGE_BYTES;
use crate::DesktopError;

const TRANSPORT: &str = "sugarsubstitute.bulk";
const VERSION: u32 = 1;

/// One 128-bit single-use capability, rendered as 32 lowercase hexadecimal digits.
///
/// Debug output is redacted: tokens must never reach diagnostics.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct Token([u8; 16]);

impl Token {
    /// Adopt bytes from the operating system's cryptographic generator.
    pub(crate) const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Accept only the exact canonical rendering this host issues.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let digits = text.as_bytes();
        if digits.len() != 32 {
            return None;
        }
        let mut bytes = [0; 16];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = (nibble(digits[2 * index])? << 4) | nibble(digits[2 * index + 1])?;
        }
        Some(Self(bytes))
    }

    /// Render the canonical lowercase form used in URLs and envelopes.
    pub(crate) fn hex(self) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        self.0
            .iter()
            .flat_map(|byte| [DIGITS[usize::from(byte >> 4)], DIGITS[usize::from(byte & 15)]])
            .map(char::from)
            .collect()
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Token(..)")
    }
}

/// Decode one lowercase hexadecimal digit; uppercase is a different, rejected spelling.
fn nibble(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        _ => None,
    }
}

/// The private envelope that names a bulk body in place of an inline message.
///
/// Wire shape: `{"transport":"sugarsubstitute.bulk","version":1,"token":"<32 hex>","bytes":N}`.
/// Public protocol messages always carry `protocol` and never `transport`, so neither shape
/// can decode as the other.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Envelope {
    pub(crate) token: Token,
    /// Exact UTF-8 length of the named body.
    pub(crate) bytes: usize,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct EnvelopeWire<'a> {
    transport: &'a str,
    version: u32,
    token: &'a str,
    bytes: u64,
}

impl Envelope {
    /// Decode the exact envelope shape; anything else remains a candidate public message.
    pub(crate) fn decode(payload: &str) -> Option<Self> {
        let wire: EnvelopeWire<'_> = serde_json::from_str(payload).ok()?;
        let bytes = usize::try_from(wire.bytes).ok()?;
        (wire.transport == TRANSPORT
            && wire.version == VERSION
            && (1..=MAX_MESSAGE_BYTES).contains(&bytes))
        .then_some(())?;
        Some(Self { token: Token::parse(wire.token)?, bytes })
    }

    /// Encode the canonical envelope delivered through the inline path.
    pub(crate) fn encode(self) -> Result<Vec<u8>, DesktopError> {
        let token = self.token.hex();
        serde_json::to_vec(&EnvelopeWire {
            transport: TRANSPORT,
            version: VERSION,
            token: &token,
            bytes: self.bytes as u64,
        })
        .map_err(|cause| DesktopError::new("encode_bulk_envelope", cause))
    }
}

/// The two operations the bulk origin serves; everything else is answered with 404.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Route {
    /// `GET /d/<token>` retrieves one staged host message.
    Download(Token),
    /// `POST /u/<token>` supplies the body for one ordinary credit.
    Upload(Token),
}
