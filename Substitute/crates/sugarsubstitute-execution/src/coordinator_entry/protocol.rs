// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the private entry schema and borrowed decoded payload without native authority.
mod decode;
mod encode;
pub(crate) mod launch_payload;
mod raw_windows;
#[cfg(test)]
mod tests;
use super::types::{EntryTransportError, MAX_ENTRY_FRAME_BYTES};
pub(super) use decode::decode;
pub(super) use encode::encode;
#[cfg(test)]
pub(super) use encode::encoded_length;
pub(super) use encode::{bind_staged_launch, stage_launch};
use sugarsubstitute_contracts::{
    coordinator_entry::{
        EntryLaunchRequest, EntryLaunchRequestView, EntryRejection, EntryRequestId,
    },
    execution::CounterDeadline,
    lifecycle::{CoordinatorEpoch, NamespaceId, RunKey},
};
pub(super) const HEADER_BYTES: usize = 120;
const MAGIC: &[u8; 4] = b"SSCE";
/// Version 3 carries a fixed passive-originals commitment after the stored renderer preference.
/// Earlier peers reject the envelope before decoding a changed body or invoking native effects.
const VERSION: u16 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Kind {
    Hello = 1,
    Challenge = 2,
    Launch = 3,
    Accepted = 4,
    Rejected = 5,
    Closing = 6,
    Initializing = 7,
}
impl Kind {
    fn from_raw(value: u16) -> Result<Self, EntryTransportError> {
        match value {
            1 => Ok(Self::Hello),
            2 => Ok(Self::Challenge),
            3 => Ok(Self::Launch),
            4 => Ok(Self::Accepted),
            5 => Ok(Self::Rejected),
            6 => Ok(Self::Closing),
            7 => Ok(Self::Initializing),
            _ => Err(EntryTransportError::Malformed),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Header {
    pub(super) namespace: NamespaceId,
    pub(super) epoch: Option<CoordinatorEpoch>,
    pub(super) request: EntryRequestId,
    pub(super) source: CounterDeadline,
    pub(super) nonce: [u8; 32],
}
impl Header {
    fn validate(self, kind: Kind) -> Result<(), EntryTransportError> {
        let unbound = matches!(kind, Kind::Hello | Kind::Initializing);
        if !unbound && self.epoch.is_none() {
            return Err(EntryTransportError::Malformed);
        }
        let empty_nonce = self.nonce.iter().all(|byte| *byte == 0);
        if (unbound && !empty_nonce) || (!unbound && empty_nonce) {
            return Err(EntryTransportError::Malformed);
        }
        if kind == Kind::Initializing && self.epoch.is_some() {
            return Err(EntryTransportError::Malformed);
        }
        Ok(())
    }
    pub(super) fn same_source(self, other: Self) -> bool {
        self.namespace == other.namespace
            && self.request == other.request
            && self.source == other.source
    }
}

pub(super) enum Payload<'a> {
    Hello,
    Challenge,
    Launch(&'a EntryLaunchRequest),
    Accepted(RunKey),
    Rejected(EntryRejection),
    #[cfg(test)]
    Closing,
    Initializing,
}
impl Payload<'_> {
    const fn kind(&self) -> Kind {
        match self {
            Self::Hello => Kind::Hello,
            Self::Challenge => Kind::Challenge,
            Self::Launch(_) => Kind::Launch,
            Self::Accepted(_) => Kind::Accepted,
            Self::Rejected(_) => Kind::Rejected,
            #[cfg(test)]
            Self::Closing => Kind::Closing,
            Self::Initializing => Kind::Initializing,
        }
    }
}
pub(super) enum DecodedPayload<'a> {
    Hello,
    Challenge,
    Launch(EntryLaunchRequestView<'a>),
    Accepted(RunKey),
    Rejected(EntryRejection),
    Closing,
    Initializing,
}
pub(super) struct Frame<'a> {
    pub(super) header: Header,
    pub(super) payload: DecodedPayload<'a>,
}

struct Writer<'a> {
    output: &'a mut [u8],
    position: usize,
}
impl<'a> Writer<'a> {
    fn new(output: &'a mut [u8]) -> Self {
        Self { output, position: 0 }
    }
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), EntryTransportError> {
        let end = self.position.checked_add(bytes.len()).ok_or(EntryTransportError::Capacity)?;
        self.output
            .get_mut(self.position..end)
            .ok_or(EntryTransportError::Capacity)?
            .copy_from_slice(bytes);
        self.position = end;
        Ok(())
    }
    fn byte(&mut self, value: u8) -> Result<(), EntryTransportError> {
        self.bytes(&[value])
    }
    fn u16(&mut self, value: u16) -> Result<(), EntryTransportError> {
        self.bytes(&value.to_le_bytes())
    }
    fn u32(&mut self, value: usize) -> Result<(), EntryTransportError> {
        self.bytes(&u32::try_from(value).map_err(|_| EntryTransportError::Capacity)?.to_le_bytes())
    }
}
struct Reader<'a> {
    input: &'a [u8],
    position: usize,
}
impl<'a> Reader<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, position: 0 }
    }
    fn bytes(&mut self, count: usize) -> Result<&'a [u8], EntryTransportError> {
        let end = self.position.checked_add(count).ok_or(EntryTransportError::Malformed)?;
        let bytes = self.input.get(self.position..end).ok_or(EntryTransportError::Malformed)?;
        self.position = end;
        Ok(bytes)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], EntryTransportError> {
        self.bytes(N)?.try_into().map_err(|_| EntryTransportError::Malformed)
    }
    fn byte(&mut self) -> Result<u8, EntryTransportError> {
        Ok(self.array::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, EntryTransportError> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<usize, EntryTransportError> {
        usize::try_from(u32::from_le_bytes(self.array()?))
            .map_err(|_| EntryTransportError::Malformed)
    }
    fn section(&mut self) -> Result<&'a [u8], EntryTransportError> {
        let count = self.u32()?;
        self.bytes(count)
    }
    fn finished(&self) -> bool {
        self.position == self.input.len()
    }
}
