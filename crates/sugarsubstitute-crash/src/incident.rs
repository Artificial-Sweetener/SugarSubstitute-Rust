// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve bounded incident records independently of native capture and presentation.
mod codec;
mod codec_evidence;
mod codec_identity;
mod codec_ledger;
mod codec_seed;
mod codec_summary;
mod codec_symbols;
mod error;
mod evidence;

pub use codec::{
    decode_release_symbols, decode_seed, decode_summary, decode_symbol_report,
    encode_release_symbols, encode_seed, encode_summary, encode_symbol_report,
};
pub use codec_ledger::{
    IncidentFileFacts, IncidentStoreState, SummaryEvictionJournal, decode_store_state,
    encode_store_state,
};
pub use error::IncidentStoreError;
pub use evidence::{IncidentLogCode, RedactedDiagnosticFact, RedactedTextChunk};

#[cfg(test)]
mod tests;
