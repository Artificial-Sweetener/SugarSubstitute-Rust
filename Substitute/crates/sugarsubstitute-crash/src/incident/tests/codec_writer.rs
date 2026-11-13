// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check canonical output and failure custody at the concrete writer boundary.
use super::super::{IncidentStoreError, codec::Record, codec_encode};
use serde::Serialize;
use std::{error::Error, fmt, io};
type TestResult = Result<(), Box<dyn Error>>;

#[derive(Serialize)]
struct Content<'a> {
    text: &'a str,
    nullable: Option<u64>,
    count: u64,
}

#[test]
fn canonical_writer_preserves_schema_field_order_nullable_and_escaped_text() -> TestResult {
    for schema in [1, 2] {
        let content = Content { text: "a\n\"雪", nullable: None, count: u64::MAX };
        let original = serde_json::to_vec(&Record { schema_version: schema, content: &content })?;
        let mut output = Vec::new();
        codec_encode::write(schema, &content, &mut output)?;
        assert_eq!(output, original);
        assert_eq!(codec_encode::encode(schema, content, original.len())?, original);
    }
    Ok(())
}

#[derive(Debug)]
struct WriterFailure;
impl fmt::Display for WriterFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("original writer failure")
    }
}
impl Error for WriterFailure {}
struct PartialWriter {
    output: Vec<u8>,
}
impl io::Write for PartialWriter {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if self.output.len() >= 7 {
            return Err(io::Error::other(WriterFailure));
        }
        let count = input.len().min(7 - self.output.len());
        self.output.extend_from_slice(&input[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn canonical_writer_retains_actual_partial_output_and_original_io_cause() -> TestResult {
    let content = Content { text: "kept", nullable: Some(1), count: 2 };
    let original = serde_json::to_vec(&Record { schema_version: 1, content: &content })?;
    let mut writer = PartialWriter { output: Vec::new() };
    let error = codec_encode::write(1, &content, &mut writer)
        .err()
        .ok_or("writer unexpectedly succeeded")?;
    let IncidentStoreError::Codec(cause) = error else {
        return Err("original codec cause changed category".into());
    };
    assert!(cause.is_io());
    assert_eq!(cause.to_string(), "original writer failure");
    assert_eq!(writer.output, original[..7]);
    Ok(())
}

struct BrokenContent;
impl Serialize for BrokenContent {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("original serializer failure"))
    }
}

#[test]
fn standalone_encoding_preserves_serializer_cause_before_size_rejection() -> TestResult {
    let original = serde_json::to_vec(&Record { schema_version: 1, content: BrokenContent })
        .err()
        .ok_or("original serializer unexpectedly succeeded")?;
    let error = codec_encode::encode(1, BrokenContent, 0)
        .err()
        .ok_or("serializer unexpectedly succeeded")?;
    let IncidentStoreError::Codec(cause) = error else {
        return Err("serializer priority changed".into());
    };
    assert_eq!(cause.to_string(), original.to_string());
    assert_eq!(cause.classify(), original.classify());
    Ok(())
}

#[test]
fn standalone_encoding_accepts_exact_bound_and_rejects_one_byte_less() -> TestResult {
    let original = serde_json::to_vec(&Record { schema_version: 2, content: 9_u64 })?;
    assert_eq!(codec_encode::encode(2, 9_u64, original.len())?, original);
    assert!(matches!(
        codec_encode::encode(2, 9_u64, original.len() - 1),
        Err(IncidentStoreError::RecordBound)
    ));
    Ok(())
}
