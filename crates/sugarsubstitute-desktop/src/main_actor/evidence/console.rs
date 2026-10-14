// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project bounded primitive console diagnostics without materializing remote objects or stacks.
//!
//! CDP supplies RemoteObject previews and recursive asynchronous stackTrace metadata. Their
//! shape carries no readiness authority. The pinned JSON decoder validates and skips that
//! metadata iteratively within the original native byte envelope; typed fields and the
//! at-most-32 actual arguments retain their separate admission and document checks.

use super::{DesktopError, MainReadiness, readiness_error};
use crate::main_readiness_types::{ConsoleArgument, ConsoleKind, ReadinessFault};
use serde::{
    Deserialize, Deserializer,
    de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor},
};
use std::fmt;

const ARGUMENTS: usize = 32;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Console {
    execution_context_id: i64,
    #[serde(rename = "type")]
    kind: CallKind,
    args: Arguments,
}

/// Accept actual protocol call types; only error and warning classify setup diagnostics.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum CallKind {
    Log,
    Debug,
    Info,
    Error,
    Warning,
    Dir,
    Dirxml,
    Table,
    Trace,
    Clear,
    StartGroup,
    StartGroupCollapsed,
    EndGroup,
    Assert,
    Profile,
    ProfileEnd,
    Count,
    TimeEnd,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum RemoteKind {
    Object,
    Function,
    Undefined,
    String,
    Number,
    Boolean,
    Symbol,
    Bigint,
}

#[derive(Deserialize)]
struct Argument {
    #[serde(rename = "type")]
    kind: RemoteKind,
    value: Option<Primitive>,
}

/// Allocate only actual string values; nested JSON values are validated and discarded.
enum Primitive {
    String(String),
    Other,
}

impl<'de> Deserialize<'de> for Primitive {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct PrimitiveVisitor;
        impl<'de> Visitor<'de> for PrimitiveVisitor {
            type Value = Primitive;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON console value")
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Primitive, E> {
                Ok(Primitive::String(value.into()))
            }
            fn visit_string<E: de::Error>(self, value: String) -> Result<Primitive, E> {
                Ok(Primitive::String(value))
            }
            fn visit_bool<E: de::Error>(self, _: bool) -> Result<Primitive, E> {
                Ok(Primitive::Other)
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<Primitive, E> {
                Ok(Primitive::Other)
            }
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<Primitive, E> {
                Ok(Primitive::Other)
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<Primitive, E> {
                Ok(Primitive::Other)
            }
            fn visit_unit<E: de::Error>(self) -> Result<Primitive, E> {
                Ok(Primitive::Other)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut values: A) -> Result<Primitive, A::Error> {
                while values.next_element::<IgnoredAny>()?.is_some() {}
                Ok(Primitive::Other)
            }
            fn visit_map<A: MapAccess<'de>>(self, mut values: A) -> Result<Primitive, A::Error> {
                while values.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                Ok(Primitive::Other)
            }
        }
        decoder.deserialize_any(PrimitiveVisitor)
    }
}

/// Fixed admission before decoding each argument, independent of unconsumed metadata size.
struct Arguments([Option<Argument>; ARGUMENTS]);

impl<'de> Deserialize<'de> for Arguments {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct ArgumentsVisitor;
        impl<'de> Visitor<'de> for ArgumentsVisitor {
            type Value = Arguments;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("at most 32 console arguments")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut values: A) -> Result<Arguments, A::Error> {
                let mut arguments = Arguments(std::array::from_fn(|_| None));
                for slot in &mut arguments.0 {
                    let Some(argument) = values.next_element()? else { return Ok(arguments) };
                    *slot = Some(argument);
                }
                if values.next_element::<IgnoredAny>()?.is_some() {
                    return Err(de::Error::custom("console argument admission exceeded"));
                }
                Ok(arguments)
            }
        }
        decoder.deserialize_seq(ArgumentsVisitor)
    }
}

/// Preserve the current document's authority and exact primitive failures after projection.
pub(super) fn consume(owner: &mut MainReadiness, payload: &str) -> Result<(), DesktopError> {
    if payload.len() > crate::main_events::PAYLOAD_BYTES {
        return Err(DesktopError::message(
            "decode_main_observation",
            "native JSON byte bound exceeded",
        ));
    }
    let event: Console = serde_json::from_str(payload)
        .map_err(|cause| DesktopError::new("decode_main_observation", cause))?;
    let kind = match event.kind {
        CallKind::Error => ConsoleKind::Error,
        CallKind::Warning => ConsoleKind::Warning,
        _ => ConsoleKind::Other,
    };
    // Even an empty console call must belong to the current admitted native context.
    owner
        .console(event.execution_context_id, kind, ConsoleArgument::Other)
        .map_err(readiness_error)?;
    for argument in event.args.0.iter().flatten() {
        let primitive = match (&argument.kind, &argument.value) {
            (RemoteKind::String, Some(Primitive::String(value))) => {
                ConsoleArgument::PrimitiveString(value)
            }
            (RemoteKind::String, _) => {
                return Err(readiness_error(ReadinessFault::MalformedObservation));
            }
            _ => ConsoleArgument::Other,
        };
        owner.console(event.execution_context_id, kind, primitive).map_err(readiness_error)?;
    }
    Ok(())
}
