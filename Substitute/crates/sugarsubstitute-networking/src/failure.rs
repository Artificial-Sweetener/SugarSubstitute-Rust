// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve native causes and operation identities without formatting private request data.
/// Original transport failure.
#[derive(Debug)]
pub struct Failure {
    pub(crate) stage: &'static str,
    pub(crate) cause: Cause,
}
/// Original failure category, retained for feature-owned retry classification.
#[derive(Debug)]
pub enum Cause {
    /// Exact native HRESULT, without a formatted replacement.
    Native(windows::core::Error),
    /// Invalid policy input, cancellation or finite deadline failure.
    Io(std::io::Error),
}
impl Failure {
    /// Consume operation identity and original cause; no request/response bytes are exposed.
    pub fn into_parts(self) -> (&'static str, Cause) {
        (self.stage, self.cause)
    }
    pub(crate) fn native(stage: &'static str, cause: windows::core::Error) -> Self {
        Self { stage, cause: Cause::Native(cause) }
    }
    pub(crate) fn invalid(stage: &'static str) -> Self {
        Self {
            stage,
            cause: Cause::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, stage)),
        }
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP transport {} failed", self.stage)
    }
}
impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match &self.cause {
            Cause::Native(cause) => cause,
            Cause::Io(cause) => cause,
        })
    }
}

impl From<Failure> for std::io::Error {
    fn from(cause: Failure) -> Self {
        let kind = match &cause.cause {
            Cause::Io(error) => error.kind(),
            Cause::Native(_) => std::io::ErrorKind::Other,
        };
        Self::new(kind, cause)
    }
}
