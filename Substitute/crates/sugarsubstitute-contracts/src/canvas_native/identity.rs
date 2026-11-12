// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate the bounded identities, keys and values the native canvas protocol exchanges.
//!
//! Every value here is correlation or presentation data. None of it names a path, grants
//! a document, or authorizes a native operation; the canvas owner resolves each identity
//! against its own state.

use super::{CanvasNativeError, wire};
use std::fmt;

/// Define one text value bounded in UTF-16 code units, matching the generated validators.
macro_rules! bounded_text {
    ($(#[$doc:meta])* $name:ident, $maximum:literal) => {
        $(#[$doc])*
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// Largest admitted length in UTF-16 code units, as the shared schema counts it.
            pub const MAX_UTF16: usize = $maximum;

            /// Adopt text within the shared bound; empty text is valid.
            ///
            /// # Errors
            /// Rejects text longer than [`Self::MAX_UTF16`] UTF-16 code units.
            pub fn new(value: impl Into<String>) -> Result<Self, CanvasNativeError> {
                let value = value.into();
                if value.encode_utf16().count() > Self::MAX_UTF16 {
                    return Err(CanvasNativeError::Invalid);
                }
                Ok(Self(value))
            }

            /// Borrow the exact text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

bounded_text!(
    /// One host-assigned input or output document identity.
    CanvasDocumentId,
    128
);
bounded_text!(
    /// One host-assigned layer identity, including mask layers.
    CanvasLayerId,
    128
);
bounded_text!(
    /// An opaque host revision token for geometry or content; only equality is meaningful.
    CanvasRevision,
    512
);
bounded_text!(
    /// A user-visible layer name, preserved exactly.
    CanvasLayerName,
    256
);
bounded_text!(
    /// SugarCubes' workflow identity, such as `workflow-<uuid>` or the shared `workflow-none`.
    CanvasWorkflowKey,
    256
);
bounded_text!(
    /// SugarCubes' stable key for one workflow input surface.
    CanvasSurfaceKey,
    256
);
bounded_text!(
    /// SugarCubes' stable key for one mask of a workflow surface.
    CanvasMaskKey,
    256
);
bounded_text!(
    /// A SugarCubes-authored surface or mask label, preserved exactly.
    CanvasLabel,
    256
);
bounded_text!(
    /// Developer-facing failure detail; SugarCubes owns any user-facing wording.
    CanvasFailureMessage,
    512
);

/// Largest document side, in document pixels.
pub const MAX_CANVAS_DOCUMENT_SIDE: u32 = 300_000;

/// A document or resize extent in document pixels; both sides are `1..=300000`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CanvasDimensions {
    width: u32,
    height: u32,
}

impl CanvasDimensions {
    /// Adopt an extent within the shared document bound.
    ///
    /// # Errors
    /// Rejects a zero side or a side above [`MAX_CANVAS_DOCUMENT_SIDE`].
    pub fn new(width: u32, height: u32) -> Result<Self, CanvasNativeError> {
        let side = 1..=MAX_CANVAS_DOCUMENT_SIDE;
        if !side.contains(&width) || !side.contains(&height) {
            return Err(CanvasNativeError::Invalid);
        }
        Ok(Self { width, height })
    }

    /// Return the width in document pixels.
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Return the height in document pixels.
    pub const fn height(self) -> u32 {
        self.height
    }

    pub(super) fn from_wire(width: u64, height: u64) -> Result<Self, CanvasNativeError> {
        Self::new(
            u32::try_from(width).map_err(|_| CanvasNativeError::Invalid)?,
            u32::try_from(height).map_err(|_| CanvasNativeError::Invalid)?,
        )
    }
}

/// The document SugarCubes addresses: identity, geometry revision and extent.
///
/// A geometry revision changes whenever the document's extent or placement changes, so a
/// request carrying an older revision is stale.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasDocument {
    /// Host-assigned document identity.
    pub id: CanvasDocumentId,
    /// Opaque revision of the document's geometry.
    pub geometry_revision: CanvasRevision,
    /// Document extent in document pixels.
    pub size: CanvasDimensions,
}

impl CanvasDocument {
    pub(super) fn from_wire(value: wire::CanvasDocument) -> Result<Self, CanvasNativeError> {
        Ok(Self {
            id: CanvasDocumentId::new(value.id)?,
            geometry_revision: CanvasRevision::new(value.geometry_revision)?,
            size: CanvasDimensions::from_wire(value.width, value.height)?,
        })
    }

    pub(super) fn to_wire(&self) -> wire::CanvasDocument {
        wire::CanvasDocument {
            id: self.id.as_str().into(),
            geometry_revision: self.geometry_revision.as_str().into(),
            width: self.size.width().into(),
            height: self.size.height().into(),
        }
    }
}

/// An opaque sRGB colour exchanged as `#rrggbb`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CanvasColor {
    /// Red channel.
    pub red: u8,
    /// Green channel.
    pub green: u8,
    /// Blue channel.
    pub blue: u8,
}

impl CanvasColor {
    /// Parse `#rrggbb` with hexadecimal digits of either case.
    ///
    /// # Errors
    /// Rejects any other spelling, including named colours and alpha forms.
    pub fn parse(value: &str) -> Result<Self, CanvasNativeError> {
        let digits = value.strip_prefix('#').filter(|digits| digits.len() == 6);
        let channel = |index: usize| {
            digits
                .and_then(|digits| digits.get(index..index + 2))
                .filter(|pair| pair.bytes().all(|byte| byte.is_ascii_hexdigit()))
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                .ok_or(CanvasNativeError::Invalid)
        };
        Ok(Self { red: channel(0)?, green: channel(2)?, blue: channel(4)? })
    }

    /// Spell the colour as lowercase `#rrggbb`.
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
    }
}

/// An overlay opacity in `0.0..=1.0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasOpacity(f64);

impl CanvasOpacity {
    /// Adopt a finite opacity within the unit interval.
    ///
    /// # Errors
    /// Rejects values outside `0.0..=1.0`, including NaN.
    pub fn new(value: f64) -> Result<Self, CanvasNativeError> {
        if !(0.0..=1.0).contains(&value) {
            return Err(CanvasNativeError::Invalid);
        }
        Ok(Self(value))
    }

    /// Return the opacity exactly as received.
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// The shared failure statuses every native reply family uses.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasFailureStatus {
    /// The host cannot service the request now, for example without an input document.
    Unavailable,
    /// The host does not implement the request.
    Unsupported,
    /// The request addressed a changed or retired document or revision.
    Stale,
    /// The request was cancelled before completion.
    Cancelled,
    /// The host failed while servicing the request.
    Error,
}

impl CanvasFailureStatus {
    /// Return the exact shared token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Unsupported => "unsupported",
            Self::Stale => "stale",
            Self::Cancelled => "cancelled",
            Self::Error => "error",
        }
    }
}

/// A typed failure outcome with bounded developer detail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasFailure {
    /// Why the request did not succeed.
    pub status: CanvasFailureStatus,
    /// Developer-facing detail; empty is valid.
    pub message: CanvasFailureMessage,
}

impl CanvasFailure {
    /// Report `status` without detail.
    pub fn new(status: CanvasFailureStatus) -> Self {
        Self { status, message: CanvasFailureMessage(String::new()) }
    }

    pub(super) fn to_wire(&self) -> wire::CanvasFailure {
        wire::CanvasFailure {
            status: self.status.as_str().into(),
            message: self.message.as_str().into(),
        }
    }
}
