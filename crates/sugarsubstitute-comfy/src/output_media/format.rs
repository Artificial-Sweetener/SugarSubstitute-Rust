// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify original artifact transport independently of native still-image decoding.

/// Encoded output format with an exact HTTP content type and bounded framing checks.
/// Native image decoding remains with its PNG/JPEG decoder; other artifacts use browser media.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComfyOutputFormat {
    /// PNG or animated PNG served as `image/png`.
    Png,
    /// JPEG served as `image/jpeg`.
    Jpeg,
    /// WebP, including animation, served as `image/webp`.
    Webp,
    /// GIF served as `image/gif`.
    Gif,
    /// MP4 video served as `video/mp4`.
    Mp4,
    /// WebM video served as `video/webm`.
    Webm,
    /// Explicit `.apng` served as `image/apng` by the backend MIME registry.
    Apng,
}

impl ComfyOutputFormat {
    /// Classify only the formats consumed by the existing native still-image decoder.
    pub fn from_filename(filename: &str) -> Option<Self> {
        let format = Self::from_artifact_filename(filename)?;
        matches!(format, Self::Png | Self::Jpeg).then_some(format)
    }

    /// Classify supported original artifacts without claiming that native image decoding supports them.
    pub fn from_artifact_filename(filename: &str) -> Option<Self> {
        let leaf = filename.rsplit(['/', '\\']).next()?;
        let (stem, extension) = leaf.rsplit_once('.')?;
        if stem.is_empty() {
            return None;
        }
        match extension.to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "webp" => Some(Self::Webp),
            "gif" => Some(Self::Gif),
            "mp4" => Some(Self::Mp4),
            "webm" => Some(Self::Webm),
            "apng" => Some(Self::Apng),
            _ => None,
        }
    }

    /// Return the backend's exact encoded media type, never a requested conversion format.
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            Self::Gif => "image/gif",
            Self::Mp4 => "video/mp4",
            Self::Webm => "video/webm",
            Self::Apng => "image/apng",
        }
    }

    /// Return the minimum header-bearing encoded length before admitting a body.
    /// The artifact's decoder and verified digest remain independent validation boundaries.
    pub fn minimum_bytes(self) -> u64 {
        match self {
            Self::Png | Self::Apng => 33,
            Self::Jpeg => 27,
            Self::Webp => 12,
            Self::Gif => 13,
            Self::Mp4 => 12,
            Self::Webm => 8,
        }
    }

    /// Return guaranteed common leading bytes; MP4 has an offset marker and no fixed leading bytes.
    /// Transport admission uses `matches_prefix` to validate offset and variant markers as well.
    pub fn signature(self) -> &'static [u8] {
        match self {
            Self::Png | Self::Apng => b"\x89PNG\r\n\x1a\n",
            Self::Jpeg => &[0xff, 0xd8, 0xff],
            Self::Webp => b"RIFF",
            Self::Gif => b"GIF",
            Self::Mp4 => b"",
            Self::Webm => b"\x1a\x45\xdf\xa3",
        }
    }

    /// Require the complete bounded format marker, including offset container signatures.
    pub fn matches_prefix(self, bytes: &[u8]) -> bool {
        match self {
            Self::Webp => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
            Self::Gif => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
            Self::Mp4 => bytes.get(4..8) == Some(b"ftyp"),
            _ => bytes.starts_with(self.signature()),
        }
    }

    /// Name one original cache stream by its encoded container, independently of presentation.
    #[cfg(windows)]
    pub(super) fn entry_name(self) -> &'static str {
        match self {
            Self::Png | Self::Apng => "output.png",
            Self::Jpeg => "output.jpg",
            Self::Webp => "output.webp",
            Self::Gif => "output.gif",
            Self::Mp4 => "output.mp4",
            Self::Webm => "output.webm",
        }
    }
}
