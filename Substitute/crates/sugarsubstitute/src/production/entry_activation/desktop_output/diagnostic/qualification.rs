// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode schema-one compact observations; bytes and launch facts confer no authority.
use sugarsubstitute_desktop::qualification_trace::{
    Cause, NativeFailure, NativePhase, PngProof, Progress, native_operation,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum Detail {
    Progress(Progress),
    Failed(Cause),
    NativeFailed(NativeFailure),
    Persisted(PngProof),
    Finished,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct Qualification {
    pub(in super::super) launch: u64,
    pub(in super::super) navigation: u64,
    pub(in super::super) detail: Detail,
}
/// Accept the actual compact producer target and complete ordered fields only.
pub(super) fn parse(bytes: &[u8]) -> Option<Qualification> {
    if bytes.len() > 512 {
        return None;
    }
    let line = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
    let (prefix, fields) = line.split_once("operation=\"child_native_qualification\" ")?;
    if prefix.contains("operation=")
        || !prefix.ends_with("sugarsubstitute_desktop::qualification_trace: ")
    {
        return None;
    }
    let mut fields = fields.split_ascii_whitespace();
    if fields.next()? != "schema=1" {
        return None;
    }
    let stage = fields.next()?.strip_prefix("stage=")?;
    let main = match fields.next()? {
        "main=true" => true,
        "main=false" => false,
        _ => return None,
    };
    let launch = number(fields.next()?, "launch=")?;
    let navigation = number(fields.next()?, "navigation=")?;
    if main && (launch == 0 || navigation == 0) || !main && (launch != 0 || navigation != 0) {
        return None;
    }
    let detail = if let Some(progress) = Progress::parse(stage) {
        if !main {
            return None;
        }
        Detail::Progress(progress)
    } else {
        match stage {
            "\"PreviewFailed\"" => {
                Detail::Failed(Cause::parse(fields.next()?.strip_prefix("cause=")?)?)
            }
            "\"NativePreviewFailed\"" if main => {
                let phase = NativePhase::parse(fields.next()?.strip_prefix("phase=")?)?;
                let operation = native_operation(
                    fields.next()?.strip_prefix("native_operation=\"")?.strip_suffix('"')?,
                )?;
                let has = match fields.next()? {
                    "has_hresult=true" => true,
                    "has_hresult=false" => false,
                    _ => return None,
                };
                let raw = fields.next()?.strip_prefix("hresult=")?;
                let code = raw.parse::<i32>().ok()?;
                if has && (code >= 0 || !raw.starts_with('-') || raw.starts_with("-0"))
                    || !has && raw != "0"
                {
                    return None;
                }
                Detail::NativeFailed(NativeFailure {
                    phase,
                    operation,
                    hresult: has.then_some(code),
                })
            }
            "\"NativeFinished\"" if !main => Detail::Finished,
            "\"PreviewPersisted\"" if main => {
                let bytes = number(fields.next()?, "png_bytes=")?;
                if !(1..=2 * 1024 * 1024).contains(&bytes) {
                    return None;
                }
                let hex = fields.next()?.strip_prefix("png_sha256=")?;
                if hex.len() != 64 {
                    return None;
                }
                let mut digest = [0; 32];
                for (slot, pair) in digest.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
                    *slot = nibble(pair[0])?.checked_mul(16)?.checked_add(nibble(pair[1])?)?;
                }
                let native_navigation = number(fields.next()?, "native_navigation=")?;
                let width = u32::try_from(number(fields.next()?, "viewport_width=")?).ok()?;
                let height = u32::try_from(number(fields.next()?, "viewport_height=")?).ok()?;
                let dpi = u32::try_from(number(fields.next()?, "dpi=")?).ok()?;
                if native_navigation == 0 || width == 0 || height == 0 || dpi == 0 {
                    return None;
                }
                Detail::Persisted(PngProof { bytes, digest, native_navigation, width, height, dpi })
            }
            _ => return None,
        }
    };
    if fields.next().is_some() {
        return None;
    }
    Some(Qualification { launch, navigation, detail })
}
fn number(field: &str, prefix: &str) -> Option<u64> {
    let value = field.strip_prefix(prefix)?;
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}
fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
