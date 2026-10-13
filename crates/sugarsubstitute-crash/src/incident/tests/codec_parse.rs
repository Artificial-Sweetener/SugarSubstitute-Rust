// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize real serde forwarding, bounded rejected diagnostics and original parser causes.
use super::super::codec_parse::{self, Bounded};
use super::*;
use serde::{Deserialize, de::Visitor};
use std::{fmt, marker::PhantomData};

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Closed {
    number: u32,
}
#[derive(Debug, Deserialize, PartialEq)]
enum Forms {
    Unit,
    Newtype(u32),
    Tuple(u32, String),
    Struct { number: u32 },
}
#[derive(Debug, Deserialize, PartialEq)]
struct Wrapped(u32);
struct StringDispatch;
impl<'de> Deserialize<'de> for StringDispatch {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Own;
        impl<'de> Visitor<'de> for Own {
            type Value = StringDispatch;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("owned string")
            }
            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<Self::Value, E> {
                Err(E::custom("owned-string fixture received borrowed text"))
            }
            fn visit_string<E: serde::de::Error>(self, text: String) -> Result<Self::Value, E> {
                if text == "owned" {
                    Ok(StringDispatch)
                } else {
                    Err(E::custom("unexpected owned text"))
                }
            }
        }
        d.deserialize_string(Own)
    }
}
#[derive(Debug)]
struct CauseLeaf;
impl fmt::Display for CauseLeaf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UNEXPECTED_IO_CAUSE")
    }
}
impl Error for CauseLeaf {}
#[derive(Debug)]
struct CauseLink(CauseLeaf);
impl fmt::Display for CauseLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Error for CauseLink {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}
struct FailedSource(serde_json::Error);
impl<'de> serde::Deserializer<'de> for FailedSource {
    type Error = serde_json::Error;
    fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Self::Error> {
        Err(self.0)
    }
    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct
        map struct enum identifier ignored_any
    }
}
#[test]
fn borrowed_owned_and_escaped_scalar_dispatch_remains_genuine() -> Result<(), Box<dyn Error>> {
    let input = br#""borrowed""#;
    let text = codec_parse::parse::<&str>(input)?;
    assert_eq!(text, "borrowed");
    assert_eq!(text.as_ptr(), input[1..].as_ptr());
    assert_eq!(codec_parse::parse::<String>(br#""\u0065scaped""#)?, "escaped");
    StringDispatch::deserialize(Bounded(
        serde::de::value::StringDeserializer::<serde_json::Error>::new("owned".into()),
    ))?;
    let bytes: &[u8] =
        Deserialize::deserialize(Bounded(serde::de::value::BorrowedBytesDeserializer::<
            serde_json::Error,
        >::new(b"bytes")))?;
    assert_eq!(bytes, b"bytes");
    let original = serde_json::Error::io(std::io::Error::other(CauseLink(CauseLeaf)));
    let expected_source = original.source().map(ToString::to_string);
    let expected_identity = original.source().map(|cause| std::ptr::from_ref(cause).cast::<()>());
    assert!(expected_source.is_some());
    let expected_display = original.to_string();
    let error = String::deserialize(Bounded(FailedSource(original))).err().ok_or("failure lost")?;
    assert_eq!(error.to_string(), expected_display);
    assert_eq!(error.source().map(ToString::to_string), expected_source);
    assert_eq!(
        error.source().map(|cause| std::ptr::from_ref(cause).cast::<()>()),
        expected_identity
    );
    Ok(())
}
#[test]
fn enum_map_sequence_option_newtype_and_seed_paths_forward() -> Result<(), Box<dyn Error>> {
    for (json, expected) in [
        (r#""Unit""#, Forms::Unit),
        (r#"{"Newtype":3}"#, Forms::Newtype(3)),
        (r#"{"Tuple":[4,"text"]}"#, Forms::Tuple(4, "text".into())),
        (r#"{"Struct":{"number":5}}"#, Forms::Struct { number: 5 }),
    ] {
        assert_eq!(codec_parse::parse::<Forms>(json.as_bytes())?, expected);
    }
    assert_eq!(codec_parse::parse::<Wrapped>(b"8")?, Wrapped(8));
    assert_eq!(codec_parse::parse::<Option<u32>>(b"null")?, None);
    assert_eq!(codec_parse::parse::<Option<u32>>(b"9")?, Some(9));
    assert_eq!(codec_parse::parse_seed(b"[1,2,3]", PhantomData::<Vec<u8>>)?, [1, 2, 3]);
    assert_eq!(codec_parse::parse::<Closed>(br#"{"number":10}"#)?, Closed { number: 10 });
    Ok(())
}
#[test]
fn rejected_maximum_identifiers_have_bounded_actual_json_causes() -> Result<(), Box<dyn Error>> {
    for name in [
        "\u{7f}".repeat(65000),
        "\u{85}".repeat(32000),
        "\u{202e}".repeat(21000),
        "\\u007f".repeat(10800),
    ] {
        let json = format!("{{\"{name}\":[");
        assert!(json.len() <= 65_536);
        let error = codec_parse::parse::<Closed>(json.as_bytes())
            .err()
            .ok_or("unknown identifier accepted")?;
        assert_eq!(error.classify(), serde_json::error::Category::Data);
        assert!(error.to_string().starts_with("Unknown incident field at line 1 column "));
        assert!(error.to_string().len() < 80);
        assert!(error.column() > 0);
        let tag = format!("\"{name}\"");
        let error =
            codec_parse::parse::<Forms>(tag.as_bytes()).err().ok_or("unknown tag accepted")?;
        assert!(error.to_string().starts_with("Unknown incident variant at line 1 column "));
        assert!(error.to_string().len() < 80);
    }
    Ok(())
}
#[test]
fn unknown_escaped_names_reject_before_malformed_values_and_fix_actual_position()
-> Result<(), Box<dyn Error>> {
    let json = b"{\n  \"\\u0050RIVATE\": [";
    let baseline = serde_json::from_slice::<Closed>(json).err().ok_or("baseline accepted")?;
    let bounded = codec_parse::parse::<Closed>(json).err().ok_or("bounded accepted")?;
    assert_eq!(
        (bounded.line(), bounded.column(), bounded.classify()),
        (baseline.line(), baseline.column(), baseline.classify())
    );
    assert!(!bounded.to_string().contains("PRIVATE"));
    assert!(bounded.to_string().contains("Unknown incident field"));
    let json = br#"{"number":1,"number":["#;
    let baseline = serde_json::from_slice::<Closed>(json).err().ok_or("duplicate accepted")?;
    let bounded = codec_parse::parse::<Closed>(json).err().ok_or("bounded duplicate accepted")?;
    assert_eq!(bounded.to_string(), baseline.to_string());
    Ok(())
}
#[test]
fn large_wrong_type_text_is_bounded_without_changing_short_diagnostics()
-> Result<(), Box<dyn Error>> {
    for json in
        [br#"{"number":"short"}"#.as_slice(), br#"{"number":-1}"#, br#"{"number":4294967296}"#]
    {
        let baseline = serde_json::from_slice::<Closed>(json).err().ok_or("baseline accepted")?;
        let bounded = codec_parse::parse::<Closed>(json).err().ok_or("bounded accepted")?;
        assert_eq!(bounded.to_string(), baseline.to_string());
    }
    let json = format!("{{\"number\":\"{}\"}}", "\u{7f}".repeat(64000));
    let error = codec_parse::parse::<Closed>(json.as_bytes()).err().ok_or("wrong type accepted")?;
    assert!(
        error.to_string().starts_with("Invalid incident text type at line 1 column "),
        "{}",
        error
    );
    assert!(error.to_string().len() < 80);
    Ok(())
}
#[test]
fn parser_syntax_eof_numeric_and_end_errors_remain_exact() -> Result<(), Box<dyn Error>> {
    for json in [
        b"{\"number\":".as_slice(),
        b"{\"number\":01}",
        b"{\"number\":true,}",
        b"{\"number\":1} false",
        b"{\"number\":1}\xff",
    ] {
        let baseline = serde_json::from_slice::<Closed>(json).err().ok_or("baseline accepted")?;
        let bounded = codec_parse::parse::<Closed>(json).err().ok_or("bounded accepted")?;
        assert_eq!(bounded.to_string(), baseline.to_string());
        assert_eq!(
            (bounded.line(), bounded.column(), bounded.classify()),
            (baseline.line(), baseline.column(), baseline.classify())
        );
    }
    Ok(())
}
#[test]
fn owner_retains_real_bounded_cause_and_syntax_causes() -> Result<(), Box<dyn Error>> {
    let canonical =
        String::from_utf8(encode_summary(&StoredIncidentSummary::new(summary()?, None)?)?)?;
    let malicious = canonical.replacen("{", &format!("{{\"{}\":[", "\u{7f}".repeat(62000)), 1);
    assert!(malicious.len() <= 65_536);
    let error = decode_summary(malicious.as_bytes()).err().ok_or("unknown accepted")?;
    let cause = error
        .source()
        .and_then(|c| c.downcast_ref::<serde_json::Error>())
        .ok_or("original JSON error lost")?;
    assert!(cause.to_string().contains("Unknown incident field"));
    assert!(cause.to_string().len() < 80);
    assert!(!format!("{error} {error:?}").contains('\u{7f}'));
    Ok(())
}

#[test]
fn actual_scalar_limits_preserve_numeric_and_nullable_results() -> Result<(), Box<dyn Error>> {
    for json in ["0", "18446744073709551615", "-1", "1.25", "1e309", "01", "null"] {
        let old = serde_json::from_str::<u64>(json);
        let new = codec_parse::parse::<u64>(json.as_bytes());
        match (old, new) {
            (Ok(a), Ok(b)) => assert_eq!(a, b),
            (Err(a), Err(b)) => assert_eq!(a.to_string(), b.to_string()),
            _ => return Err("numeric decoding changed".into()),
        }
    }
    for json in ["-9223372036854775808", "9223372036854775807", "9223372036854775808", "-0"] {
        let old = serde_json::from_str::<i64>(json);
        let new = codec_parse::parse::<i64>(json.as_bytes());
        match (old, new) {
            (Ok(a), Ok(b)) => assert_eq!(a, b),
            (Err(a), Err(b)) => assert_eq!(a.to_string(), b.to_string()),
            _ => return Err("signed numeric decoding changed".into()),
        }
    }
    assert_eq!(codec_parse::parse::<[u8; 3]>(b"[0,128,255]")?, [0, 128, 255]);
    codec_parse::parse::<()>(b"null")?;
    assert!(codec_parse::parse::<bool>(b"true")?);
    Ok(())
}

struct MapShape;
impl<'de> Deserialize<'de> for MapShape {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Both;
        impl<'de> Visitor<'de> for Both {
            type Value = MapShape;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a requested map")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                _: A,
            ) -> Result<Self::Value, A::Error> {
                Ok(MapShape)
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                _: A,
            ) -> Result<Self::Value, A::Error> {
                Ok(MapShape)
            }
        }
        d.deserialize_map(Both)
    }
}
#[test]
fn requested_shape_and_struct_sequence_compatibility_are_preserved() -> Result<(), Box<dyn Error>> {
    codec_parse::parse::<MapShape>(b"{}")?;
    let baseline =
        serde_json::from_slice::<MapShape>(b"[]").err().ok_or("baseline map accepted seq")?;
    let bounded = codec_parse::parse::<MapShape>(b"[]").err().ok_or("map accepted seq")?;
    assert_eq!(baseline.classify(), bounded.classify());
    assert_eq!(baseline.line(), bounded.line());
    assert_eq!((baseline.column(), bounded.column()), (0, 2));
    assert!(baseline.to_string().starts_with("invalid type: sequence, expected a requested map"));
    assert!(bounded.to_string().starts_with("invalid type: sequence, expected a requested map"));
    let baseline =
        serde_json::from_slice::<Vec<u32>>(b"{}").err().ok_or("baseline seq accepted map")?;
    let bounded = codec_parse::parse::<Vec<u32>>(b"{}").err().ok_or("seq accepted map")?;
    assert!(bounded.to_string().starts_with("invalid type: map, expected a sequence"));
    assert_eq!((baseline.column(), bounded.column()), (0, 2));
    for (json, expected) in [(b"[1]".as_slice(), 1), (b"[ ]", 3), (b"[", 1)] {
        let original =
            serde_json::from_slice::<MapShape>(json).err().ok_or("original shape accepted")?;
        let candidate =
            codec_parse::parse::<MapShape>(json).err().ok_or("candidate shape accepted")?;
        assert_eq!((original.column(), candidate.column()), (0, expected));
        assert_eq!(candidate.classify(), original.classify());
        assert!(
            candidate.to_string().starts_with("invalid type: sequence, expected a requested map")
        );
    }
    assert_eq!(codec_parse::parse::<Closed>(b"[7]")?, serde_json::from_slice::<Closed>(b"[7]")?);
    Ok(())
}
