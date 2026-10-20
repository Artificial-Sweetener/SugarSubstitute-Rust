// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit only readiness-relevant console diagnostics to the lifecycle callback queue.
//!
//! Native JSON remains byte-bounded. Projection retains no strings, objects or stacks;
//! escaped-string decoder scratch is covered by the callback owner's memory claim.
use serde::{
    Deserialize, Deserializer,
    de::{IgnoredAny, MapAccess, SeqAccess, Visitor},
};
use std::fmt;

/// Canonical ComfyUI diagnostic for failure of the required extension's setup hook.
pub(crate) const SETUP_FAILED: &str = "Error calling extension 'SugarCubes.UI' method 'setup'";
/// Canonical ComfyUI diagnostic for the required extension being disabled.
pub(crate) const DISABLED: &str = "Extension SugarCubes.UI is disabled.";

#[derive(Deserialize)]
struct Console<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    args: Arguments,
}

#[derive(Deserialize)]
struct Argument<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    value: Option<Message>,
}

struct Arguments(u8);
impl<'de> Deserialize<'de> for Arguments {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct ArgumentsVisitor;
        impl<'de> Visitor<'de> for ArgumentsVisitor {
            type Value = Arguments;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("native console arguments")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut values: A) -> Result<Arguments, A::Error> {
                let mut relevant = 0;
                while let Some(argument) = values.next_element::<Argument<'_>>()? {
                    if argument.kind == "string" {
                        relevant |= argument.value.map_or(0, |value| value.0);
                    }
                }
                Ok(Arguments(relevant))
            }
        }
        decoder.deserialize_seq(ArgumentsVisitor)
    }
}

struct Message(u8);
impl<'de> Deserialize<'de> for Message {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct MessageVisitor;
        impl<'de> Visitor<'de> for MessageVisitor {
            type Value = Message;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a native console value")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Message, E> {
                Ok(Message(match value {
                    SETUP_FAILED => 1,
                    DISABLED => 2,
                    _ => 0,
                }))
            }
            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Message, E> {
                Ok(Message(0))
            }
            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Message, E> {
                Ok(Message(0))
            }
            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Message, E> {
                Ok(Message(0))
            }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Message, E> {
                Ok(Message(0))
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Message, E> {
                Ok(Message(0))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut values: A) -> Result<Message, A::Error> {
                while values.next_element::<IgnoredAny>()?.is_some() {}
                Ok(Message(0))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut values: A) -> Result<Message, A::Error> {
                while values.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                Ok(Message(0))
            }
        }
        decoder.deserialize_any(MessageVisitor)
    }
}

/// Keep exact setup failures and disabled-extension warnings for the existing attributed decoder.
/// Malformed native JSON also reaches that decoder; filtering never grants readiness.
pub(crate) fn retain(payload: &str) -> bool {
    match serde_json::from_str::<Console<'_>>(payload) {
        Ok(event) => match event.kind {
            "error" => event.args.0 & 1 != 0,
            "warning" => event.args.0 & 2 != 0,
            _ => false,
        },
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn console_admission_preserves_escaped_diagnostics_and_rejects_unrelated_logging() {
        assert!(!retain(r#"{"type":"log","args":[{"type":"string","value":"ordinary output"}]}"#));
        assert!(!retain(
            r#"{"type":"warning","args":[{"type":"object","value":{"x":[1,true,null]}}]}"#
        ));
        assert!(retain(
            r#"{"type":"warning","args":[{"type":"string","value":"Extension SugarCubes\u002eUI is disabled."}]}"#
        ));
        assert!(retain(
            r#"{"type":"error","args":[{"type":"string","value":"Error calling extension 'SugarCubes.UI' method 'setup'"}]}"#
        ));
        assert!(retain("malformed native JSON"));
    }
}
