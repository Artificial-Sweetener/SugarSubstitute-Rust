// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Build the actual already-retired diagnostic capsule from a genuine admitted decoder claim.
use super::*;
/// Exercise Error ownership only; this fixture has no native reader or verified read authority.
pub(crate) fn owner(
    decode: MemoryClaim,
    first: Option<DumpReadCause>,
    secondary: Option<DumpReadCause>,
    retirement: Option<DumpReadCause>,
) -> IncidentDumpReadLease {
    assert_eq!(decode.bytes(), 786_432);
    IncidentDumpReadLease {
        owner: IncidentDumpReader {
            fields: Box::new(Fields {
                source: Vec::new(),
                stored: None,
                ledger: None,
                summary_slot: None,
                dump_slot: None,
                first,
                secondary,
                retirement,
                parser: None,
                verified: false,
                checked_current: false,
            }),
            native: None,
            _decode: decode,
        },
    }
}
